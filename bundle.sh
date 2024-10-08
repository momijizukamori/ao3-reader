#! /bin/sh

if [ "$#" -lt 1 ] ; then
	printf "Usage: %s NICKEL_MENU_ARCHIVE\n" "${0##*/}" >&2
	exit 1
fi

[ -d dist ] || ./dist.sh
[ -d bundle ] && rm -Rf bundle

NICKEL_MENU_ARCHIVE=$1
AO3R_VERSION=$(cargo pkgid -p ao3reader | cut -d '#' -f 2)

mkdir bundle
cd bundle || exit 1

if gzip -tq "$NICKEL_MENU_ARCHIVE"; then
	ln -s "$NICKEL_MENU_ARCHIVE" KoboRoot.tgz
else
	unzip "$NICKEL_MENU_ARCHIVE" KoboRoot.tgz
fi

tar -xzvf KoboRoot.tgz
rm KoboRoot.tgz
mv mnt/onboard/.adds .
rm -Rf mnt

mv ../dist .adds/ao3reader
zip -r ao3reader-"$AO3R_VERSION".zip .adds

cp ../contrib/NickelMenu/* .adds/nm

mkdir .kobo
tar -czvf .kobo/KoboRoot.tgz usr
rm -Rf usr

FIRMWARE_VERSION=$(basename "$FIRMWARE_ARCHIVE" .zip)
FIRMWARE_VERSION=${FIRMWARE_VERSION##*-}


zip -r ao3reader-bundle-"$AO3R_VERSION".zip .adds .kobo
rm -Rf .adds .kobo
