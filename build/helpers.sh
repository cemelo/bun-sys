#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

BUN_CACHE_ROOT_DEFAULT="$REPO_ROOT/vendor/bun/cache"
BUN_WORK_DIR_DEFAULT="${TMPDIR:-/tmp}/slabb-bun-embed"
BUN_CACHE_ROOT="${BUN_EMBED_CACHE_DIR:-$BUN_CACHE_ROOT_DEFAULT}"
BUN_WORK_DIR="${BUN_EMBED_WORK_DIR:-$BUN_WORK_DIR_DEFAULT}"
BUN_REPO="${BUN_EMBED_BUN_REPO:-https://github.com/oven-sh/bun.git}"
BUN_TAG="${BUN_EMBED_BUN_TAG:-bun-v1.3.3}"
DOCKER_IMAGE="${BUN_EMBED_DOCKER_IMAGE:-oven/bun:1.3.9}"
BUN_EMBED_ARCHES="${BUN_EMBED_ARCHES:-linux/arm64}"
BUN_EMBED_PIPELINE_VERSION="v2"
BUN_EMBED_FORCE_REBUILD="${BUN_EMBED_FORCE_REBUILD:-0}"
BUN_EMBED_NINJA_JOBS="${BUN_EMBED_NINJA_JOBS:-4}"

required_cache_artifacts=(
  "libbun.a"
  "libJavaScriptCore.a"
  "libWTF.a"
  "libbmalloc.a"
)

hash_stdin() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 | awk '{ print $1 }'
    return 0
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{ print $1 }'
    return 0
  fi

  openssl dgst -sha256 -r | awk '{ print $1 }'
}

patch_hash() {
  cat "$SCRIPT_DIR"/patches/*.patch | hash_stdin
}

platform_slug() {
  printf "%s" "$1" | tr '/:' '--'
}

cache_dir_for_platform() {
  printf "%s/%s\n" "$BUN_CACHE_ROOT" "$(platform_slug "$1")"
}

cache_key_for_platform() {
  local platform="$1"
  local image="${2:-$DOCKER_IMAGE}"
  printf "%s:%s:%s:%s:%s\n" \
    "$BUN_EMBED_PIPELINE_VERSION" \
    "$BUN_TAG" \
    "$platform" \
    "$image" \
    "$(patch_hash)"
}

cache_is_valid() {
  local cache_dir="$1"
  local expected_key="$2"
  local stamp_file="$cache_dir/.cache-key"

  if [ ! -f "$stamp_file" ]; then
    return 1
  fi

  if [ "$(cat "$stamp_file")" != "$expected_key" ]; then
    return 1
  fi

  for artifact in "${required_cache_artifacts[@]}"; do
    if [ ! -f "$cache_dir/$artifact" ]; then
      return 1
    fi
  done

  return 0
}

verify_requirements() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required but was not found on PATH." >&2
    exit 1
  fi

  if ! compgen -G "$SCRIPT_DIR/patches/*.patch" >/dev/null; then
    echo "No patches found in $SCRIPT_DIR/patches" >&2
    exit 1
  fi
}

build_for_platform() {
  local platform="$1"
  local slug="$2"
  local cache_dir="$3"
  local cache_key="$4"
  local work_dir="$5"
  local host_uid
  local host_gid

  host_uid="$(id -u)"
  host_gid="$(id -g)"

  mkdir -p "$work_dir" "$cache_dir"

  echo "Building Bun embed artifacts for $platform..."
  docker run --rm --platform "$platform" \
    -e DEBIAN_FRONTEND=noninteractive \
    -e BUN_TAG="$BUN_TAG" \
    -e BUN_REPO="$BUN_REPO" \
    -e HOST_UID="$host_uid" \
    -e HOST_GID="$host_gid" \
    -e NINJA_JOBS="$BUN_EMBED_NINJA_JOBS" \
    -v "$SCRIPT_DIR/patches:/patches:ro" \
    -v "$work_dir:/work" \
    -v "$cache_dir:/out" \
    "$DOCKER_IMAGE" \
    bash -lc '
      set -euo pipefail

      apt-get update -qq
      apt-get install -y -qq --no-install-recommends \
        build-essential \
        ca-certificates \
        cargo \
        clang \
        cmake \
        curl \
        git \
        golang \
        libtool \
        llvm \
        lld \
        ninja-build \
        pkg-config \
        python3 \
        ruby-full \
        rustc \
        unzip \
        wget \
        xz-utils \
        zip
      rm -rf /var/lib/apt/lists/*

      export CC=clang
      export CXX=clang++

      src_dir="/work/bun"
      if [ ! -d "$src_dir/.git" ]; then
        git clone --depth 1 --branch "$BUN_TAG" "$BUN_REPO" "$src_dir"
      fi

      cd "$src_dir"
      git fetch --depth 1 origin "refs/tags/$BUN_TAG:refs/tags/$BUN_TAG"
      git checkout -f "$BUN_TAG"
      git reset --hard "$BUN_TAG"
      git clean -fdx

      for patch in /patches/*.patch; do
        git apply --check "$patch"
        git apply "$patch"
      done

      mkdir -p cmake/sources
      bun run glob-sources
      cmake -B build -DBUN_CPP_ONLY=ON -DUSE_STATIC_SQLITE=ON -DCMAKE_BUILD_TYPE=Release -GNinja
      ninja -C build -j"$NINJA_JOBS"

      bun_archive=""
      if [ -f build/libbun.a ]; then
        bun_archive="build/libbun.a"
      elif [ -f build/libbun-profile.a ]; then
        bun_archive="build/libbun-profile.a"
      else
        echo "Could not find Bun static archive under build/" >&2
        exit 1
      fi

      jsc_archive="$(find build/cache -type f -name libJavaScriptCore.a | head -n 1)"
      if [ -z "$jsc_archive" ]; then
        echo "Could not find libJavaScriptCore.a under build/cache/" >&2
        exit 1
      fi

      webkit_lib_dir="$(dirname "$jsc_archive")"
      cp "$bun_archive" /out/libbun.a
      cp "$webkit_lib_dir/libJavaScriptCore.a" /out/libJavaScriptCore.a
      cp "$webkit_lib_dir/libWTF.a" /out/libWTF.a
      cp "$webkit_lib_dir/libbmalloc.a" /out/libbmalloc.a

      for lib in libicudata.a libicui18n.a libicuuc.a; do
        if [ -f "$webkit_lib_dir/$lib" ]; then
          cp "$webkit_lib_dir/$lib" "/out/$lib"
        fi
      done

      chown -R "$HOST_UID:$HOST_GID" /work /out
    '

  printf "%s\n" "$cache_key" > "$cache_dir/.cache-key"
  echo "Cached Linux artifacts at $cache_dir"
}

build_macos_native() {
  local slug="$1"
  local cache_dir="$2"
  local cache_key="$3"
  local src_dir="$BUN_WORK_DIR/$slug/bun"

  mkdir -p "$BUN_WORK_DIR/$slug" "$cache_dir"

  echo "Building Bun embed artifacts for macOS (native)..."

  if [ ! -d "$src_dir/.git" ]; then
    git clone --depth 1 --branch "$BUN_TAG" "$BUN_REPO" "$src_dir"
  fi

  cd "$src_dir"
  git fetch --depth 1 origin "refs/tags/$BUN_TAG:refs/tags/$BUN_TAG"
  git checkout -f "$BUN_TAG"
  git reset --hard "$BUN_TAG"
  git clean -fdx

  for patch in "$SCRIPT_DIR"/patches/*.patch; do
    git apply --check "$patch"
    git apply "$patch"
  done

  export CC=clang
  export CXX=clang++

  mkdir -p cmake/sources
  bun run glob-sources
  local osx_target
  osx_target="$(sw_vers -productVersion | cut -d. -f1)"
  cmake -B build \
    -DBUN_CPP_ONLY=ON \
    -DUSE_STATIC_SQLITE=ON \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_DEPLOYMENT_TARGET="$osx_target" \
    -GNinja
  ninja -C build -j"$BUN_EMBED_NINJA_JOBS"

  local dep_targets="boringssl brotli cares highway libdeflate lolhtml lshpack mimalloc tinycc zlib libarchive hdrhistogram zstd sqlite"
  echo "Building vendor dependencies..."
  ninja -C build $dep_targets -j"$BUN_EMBED_NINJA_JOBS"

  local bun_archive=""
  if [ -f build/libbun.a ]; then
    bun_archive="build/libbun.a"
  elif [ -f build/libbun-profile.a ]; then
    bun_archive="build/libbun-profile.a"
  else
    echo "Could not find Bun static archive under build/" >&2
    exit 1
  fi

  local jsc_archive
  jsc_archive="$(find build/cache -type f -name libJavaScriptCore.a | head -n 1)"
  if [ -z "$jsc_archive" ]; then
    echo "Could not find libJavaScriptCore.a under build/cache/" >&2
    exit 1
  fi

  local webkit_lib_dir
  webkit_lib_dir="$(dirname "$jsc_archive")"
  cp "$bun_archive" "$cache_dir/libbun.a"
  cp "$webkit_lib_dir/libJavaScriptCore.a" "$cache_dir/libJavaScriptCore.a"
  cp "$webkit_lib_dir/libWTF.a" "$cache_dir/libWTF.a"
  cp "$webkit_lib_dir/libbmalloc.a" "$cache_dir/libbmalloc.a"

  cp build/boringssl/libcrypto.a "$cache_dir/"
  cp build/boringssl/libssl.a "$cache_dir/"
  cp build/boringssl/libdecrepit.a "$cache_dir/"
  cp build/brotli/libbrotlicommon.a "$cache_dir/"
  cp build/brotli/libbrotlidec.a "$cache_dir/"
  cp build/brotli/libbrotlienc.a "$cache_dir/"
  cp build/cares/lib/libcares.a "$cache_dir/"
  cp build/hdrhistogram/src/libhdr_histogram_static.a "$cache_dir/"
  cp build/highway/libhwy.a "$cache_dir/"
  cp build/libarchive/libarchive/libarchive.a "$cache_dir/"
  cp build/libdeflate/libdeflate.a "$cache_dir/"
  cp build/lolhtml/release/liblolhtml.a "$cache_dir/"
  cp build/lshpack/libls-hpack.a "$cache_dir/"
  cp build/mimalloc/libmimalloc.a "$cache_dir/"
  cp build/sqlite/libsqlite3.a "$cache_dir/"
  cp build/tinycc/libtcc.a "$cache_dir/"
  cp build/zlib/libz.a "$cache_dir/"
  cp build/zstd/lib/libzstd.a "$cache_dir/"

  printf "%s\n" "$cache_key" > "$cache_dir/.cache-key"
  echo "Cached macOS artifacts at $cache_dir"
}

case "$(uname -s)" in
  Darwin)
    platform="macos/arm64"
    slug="$(platform_slug "$platform")"
    cache_dir="$(cache_dir_for_platform "$platform")"
    cache_key="$(cache_key_for_platform "$platform" "native")"
    mkdir -p "$BUN_CACHE_ROOT" "$BUN_WORK_DIR"

    if [ "$BUN_EMBED_FORCE_REBUILD" != "1" ] && cache_is_valid "$cache_dir" "$cache_key"; then
      echo "Cache warm for $platform at $cache_dir; skipping."
    else
      build_macos_native "$slug" "$cache_dir" "$cache_key"
    fi
    ;;
  *)
    verify_requirements
    mkdir -p "$BUN_CACHE_ROOT" "$BUN_WORK_DIR"

    IFS=' ' read -r -a target_platforms <<< "$BUN_EMBED_ARCHES"

    for platform in "${target_platforms[@]}"; do
      slug="$(platform_slug "$platform")"
      cache_dir="$(cache_dir_for_platform "$platform")"
      cache_key="$(cache_key_for_platform "$platform")"
      work_dir="$BUN_WORK_DIR/$slug"

      if [ "$BUN_EMBED_FORCE_REBUILD" != "1" ] && cache_is_valid "$cache_dir" "$cache_key"; then
        echo "Cache warm for $platform at $cache_dir; skipping."
        continue
      fi

      build_for_platform "$platform" "$slug" "$cache_dir" "$cache_key" "$work_dir"
    done
    ;;
esac

echo "Bun embed artifacts are ready under $BUN_CACHE_ROOT"
