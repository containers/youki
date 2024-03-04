# This is a temporary test-collection for validating youki runs correctly with podman in rootless mode
# This will be moved to a proper rust based test crate, similar to rust-integration tests soon

set -ex

runtime=$1

podman rm --force --ignore create-test # remove if existing

podman create --runtime $runtime --name create-test hello-world
log=$(podman start -a create-test)
echo $log | grep "This message shows that your installation appears to be working correctly"
podman rm --force --ignore create-test

rand=$(head -c 10 /dev/random | base64)

log=$(podman run --runtime $runtime fedora echo "$rand")
echo $log | grep $rand

podman rm --force --ignore exec-test
podman run -d --runtime $runtime --name exec-test busybox sleep 10m

rand=$(head -c 10 /dev/random | base64)

log=$(podman exec --runtime $runtime exec-test echo "$rand")
echo $log | grep $rand

podman kill exec-test
podman rm --force --ignore exec-test
