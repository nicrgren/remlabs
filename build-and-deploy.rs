#!/bin/bash


ARCH="armv7-unknown-linux-musleabihf"
TARGET="root@192.168.1.192"
#arch="armv7-unknown-linux-gnueabihf"

echo "Compiling..."
cross build --target "$ARCH" --release || exit 1
echo "Done"

echo "Copying target/$ARCH/release/remlabs to $TARGET:~/remlabs"
scp target/$ARCH/release/remlabs $TARGET:~/remlabs
echo "Done"

echo "Making binary executable"
ssh $TARGET "chmod +x remlabs"
echo "Done"
