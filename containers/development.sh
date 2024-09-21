#!/usr/bin/env bash


# Get the project's directory & change to it
shopt -s lastpipe
realpath "${0}" \
    | xargs dirname \
    | xargs dirname \
    | read project_directory

cd "${project_directory}"


# Log a message
#
# Usage: log (<message>)...
log() {
    printf "%s\n" "${@}"
}

# Log a warning message
#
# Usage: warning: (<message>)...
warning() {
    printf "\e[33m%s\e[m\n" "${@}"
}

# Log an error message
#
# Usage: error (<message>)...
error() {
    printf "\e[31m%s\e[m\n" "${@}"
}


# Log a help message
#
# Usage: help
help() {
    basename "${0}" \
        | read script_file

    log \
        "Usage: ${script_file} (-h | --help | command)" "" \
        "Options:" \
        "    -h --help   Log this help message" "" \
        "Commands:" \
        "    build-docker-image  Build AO3 Reader's development Docker image" \
        "    run-docker-image    Run AO3 Reader's development Docker image" \
        "    build-dependencies  Build AO3 Reader's third party dependencies" \
        "    run-emulator        Run AO3 Reader's emulator"
}

# Build AO3 Reader's development Docker image
#
# Usage: build_docker_image
build_docker_image() {
    docker build \
        --tag ao3-reader:development \
        - < ./containers/development.Dockerfile
}

# Run AO3 Reader's development Docker image
#
# Usage: run_docker_image
run_docker_image() {
    case "${XDG_SESSION_TYPE}" in
        x11)
            run_docker_image_with_x11
            ;;
        wayland)
            run_docker_image_with_wayland
            ;;
        *)
            error "AO3 Reader's development Docker image must run on X11 or Wayland"
            exit 1
            ;;
    esac
}

# Run AO3 Reader's development Docker image with X11
#
# Usage: run_docker_image_with_x11
run_docker_image_with_x11() {
    docker run \
        --env DISPLAY="${DISPLAY}" \
        --env XAUTHORITY="/tmp/.Xauthority" \
        --interactive \
        --network host \
        --rm \
        --tty \
        --user "$(id --user):$(id --group)" \
        --volume "${XAUTHORITY}:/tmp/.Xauthority" \
        --volume "${project_directory}:/opt/ao3-reader" \
        --volume /tmp/.X11-unix:/tmp/.X11-unix \
        ao3-reader:development
}

# Run AO3 Reader's development Docker image with Wayland
#
# Usage: run_docker_image_with_wayland
run_docker_image_with_wayland() {
    docker run \
        --env WAYLAND_DISPLAY="${WAYLAND_DISPLAY}" \
        --env XDG_RUNTIME_DIR=/tmp \
        --interactive \
        --rm \
        --tty \
        --user "$(id --user):$(id --group)" \
        --volume "${XDG_RUNTIME_DIR}/${WAYLAND_DISPLAY}:/tmp/${WAYLAND_DISPLAY}" \
        --volume "${project_directory}:/opt/ao3-reader" \
        ao3-reader:development
}

# Build AO3 Reader's third party dependencies
#
# Usage: build_dependencies
build_dependencies() {
    export MAKEFLAGS="-j $(busybox nproc --all)"

    cd ./thirdparty
    ./download.sh
    ./build.sh

    mkdir ../libs
    cp ./bzip2/libbz2.so ../libs
    cp ./djvulibre/libdjvu/.libs/libdjvulibre.so ../libs
    cp ./freetype2/objs/.libs/libfreetype.so ../libs
    cp ./gumbo/.libs/libgumbo.so ../libs
    cp ./gumbo/.libs/libgumbo.so ../libs/libgumbo.so.1
    cp ./harfbuzz/src/.libs/libharfbuzz.so ../libs
    cp ./jbig2dec/.libs/libjbig2dec.so ../libs
    cp ./jbig2dec/.libs/libjbig2dec.so ../libs/libjbig2dec.so.0
    cp ./libjpeg/.libs/libjpeg.so ../libs
    cp ./libjpeg/.libs/libjpeg.so ../libs/libjpeg.so.9
    cp ./libpng/.libs/libpng16.so ../libs
    cp ./mupdf/build/release/libmupdf.so ../libs
    cp ./openjpeg/build/bin/libopenjp2.so ../libs
    cp ./openjpeg/build/bin/libopenjp2.so ../libs/libopenjp2.so.7
    cp ./zlib/libz.so ../libs

    cd ../mupdf_wrapper
    ./build-kobo.sh
}

# Run AO3 Reader's emulator
#
# Usage: run_emulator
run_emulator() {
    [[ ! -f ./Settings.toml ]] && log \
        "selected-library = 0" \
	      "[[libraries]]" \
	      'name = "Example"' \
	      'path = "/opt/ao3-reader/library"' \
	      'mode = "database"' \
        > ./Settings.toml

    export LD_LIBRARY_PATH="/opt/ao3-reader/libs"
    cargo run \
        --package emulator \
        --target arm-unknown-linux-gnueabihf
}


if [[ -n "${1}" ]]; then
    case "${1}" in
        -h|--help)
            help
            ;;
        build-docker-image)
            build_docker_image
            ;;
        run-docker-image)
            run_docker_image
            ;;
        build-dependencies)
            build_dependencies
            ;;
        run-emulator)
            run_emulator
            ;;
        *)
            help
            exit 128
    esac
else
    help
    exit 128
fi
