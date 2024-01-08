#!/bin/bash

TAG=${1}

if [ -z "$TAG" ]; then
    echo "Error: No version number provided."
    exit 1
fi
VERSION=${TAG##*v}

START_MARKER="<!--youki release begin-->"
END_MARKER="<!--youki release end-->"


echo "\`\`\`console
\$ wget -qO youki-${VERSION}.tar.gz https://github.com/containers/youki/releases/download/v${VERSION}/youki-${VERSION}-\$(uname -m).tar.gz
\$ tar -zxvf youki-${VERSION}.tar.gz youki
# Maybe you need root privileges.
\$ mv youki /usr/local/bin/youki
\$ rm youki-${VERSION}.tar.gz
\`\`\`" > replace_content.txt

awk -v start="$START_MARKER" -v end="$END_MARKER" -v newfile="replace_content.txt" '
BEGIN {printing=1}
$0 ~ start {print;system("cat " newfile);printing=0}
$0 ~ end {printing=1}
printing' docs/src/user/basic_setup.md > temp.txt && mv temp.txt docs/src/user/basic_setup.md
