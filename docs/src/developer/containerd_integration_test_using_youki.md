# containerd integration test using youki

youki does not yet work with Kubernetes.
Find the cause of not supporting Kubernetes by integration test of CRI Runtime.

[pass the integration test of containerd · Issue #531 · containers/youki](https://github.com/containers/youki/issues/531)

## local

```
VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant up
vagrant ssh

# in VM
sudo -i
cd /root/go/src/github.com/containerd/containerd/
make integration
make TEST_RUNTIME=io.containerd.runc.v2 TESTFLAGS="-timeout 120m" integration
```
