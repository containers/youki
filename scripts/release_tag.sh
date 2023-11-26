#!/bin/bash

TAG=${1}

if [ -z "$TAG" ]; then
    echo "Error: No version number provided."
    exit 1
fi
VERSION=${TAG##*v}
MAJOR=${VERSION%%.*}
MINOR=${VERSION%.*}
MINOR=${MINOR#*.}
PATCH=${VERSION##*.}

START_MARKER="<!--youki release begin-->"
END_MARKER="<!--youki release end-->"


echo "\`\`\`console
\$ wget -qO youki_${VERSION}_linux.tar.gz https://github.com/containers/youki/releases/download/v${VERSION}/youki_${MAJOR}_${MINOR}_${PATCH}_linux-\$(uname -m).tar.gz
\$ tar -zxvf youki_${VERSION}_linux.tar.gz --strip-components=1
# Maybe you need root privileges.
\$ mv youki-${VERSION}/youki /usr/local/bin/youki
\$ rm -rf youki_${VERSION}_linux.tar.gz youki-${VERSION}
\`\`\`" > replace_content.txt

awk -v start="$START_MARKER" -v end="$END_MARKER" -v newfile="replace_content.txt" '
BEGIN {printing=1}
$0 ~ start {print;system("cat " newfile);printing=0}
$0 ~ end {printing=1}
printing' docs/src/user/basic_setup.md > temp.txt && mv temp.txt docs/src/user/basic_setup.md
