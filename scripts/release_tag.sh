#!/bin/bash

TAG=${1}

if [ -z "$TAG" ]; then
    echo "Error: No version number provided."
    exit 1
fi

START_MARKER="<!--youki release begin-->"
END_MARKER="<!--youki release end-->"


echo "\`\`\`console
\$ wget -qO youki_${TAG}_linux.tar.gz https://github.com/containers/youki/releases/download/v${TAG}/youki_${TAG}_linux-\$(uname -m).tar.gz
\$ tar -zxvf youki_${TAG}_linux.tar.gz --strip-components=1
# Maybe you need root privileges.
\$ mv youki-${TAG}/youki /usr/local/bin/youki
\$ rm -rf youki_${TAG}_linux.tar.gz youki-${TAG}
\`\`\`" > replace_content.txt

awk -v start="$START_MARKER" -v end="$END_MARKER" -v newfile="replace_content.txt" '
BEGIN {printing=1}
$0 ~ start {print;system("cat " newfile);printing=0}
$0 ~ end {printing=1}
printing' docs/src/user/basic_setup.md > temp.txt && mv temp.txt docs/src/user/basic_setup.md
