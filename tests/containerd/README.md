# Run

```console
$ cd $(git rev-parse --show-toplevel)
$ docker build -t youki-containerd tests/containerd  
$ docker run --privileged --net host --rm -v /tmp:/tmp:rw -v /var/lib/var-containerd:/var/lib:rw -v /sys:/sys:rw,rslave -v ${PWD}:/youki youki-containerd
```
