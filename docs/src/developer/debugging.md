# Debugging

Since Youki uses pipe and double-fork in the creating phase, it is hard to debug what happened.
You might encounter the error message, "Broken pipe ..." Unfortunately, 
this error message has only information that a child process exists with an error for some reason.

This section will give some tips to debug youki to know what happens in the child processes.

# bpftrace

[bpftrace](https://github.com/iovisor/bpftrace) is an eBPF based tool.
In the case of youki, you can catch the system calls youki issued.

For example, if you catch write system calls, you can see the log output until the middle of process.
It allows you to do something similar to print debugging.

_How to debug_
1. You need to install bpftrace, please refere to [the official documentation](https://github.com/iovisor/bpftrace/blob/master/INSTALL.md) to know how to install it.
2. Before running the process or comannd you want to debug, run the following command in another terminal.

    You need the root privilege to run it.
    ```console
    $ cd ${youki_repo}
    $ just hack-bpftrace
    ```
3. Run the command you want to debug.

_For example_

1. Run the bpftrace script.

    ```console
    $ just hack-bpftrace
    BPFTRACE_STRLEN=120 ./hack/debug.bt
    Attaching 13 probes...
    Tracing Youki syscalls... Hit Ctrl-C to end.
    TIME                 COMMAND PID      EVENT     CONTENT
    ```

2. Run the Kubernetes cluster using kind with youki

    ```console
    $ cd ${youki_repo}
    $ just test-kind
    docker buildx build --output=bin/ -f tests/k8s/Dockerfile --target kind-bin .
    ...
    Creating cluster "youki" ...
    ...
    kubectl --context=kind-youki apply -f tests/k8s/deploy.yaml
    runtimeclass.node.k8s.io/youki created
    deployment.apps/nginx-deployment created
    ...
    kubectl --context=kind-youki delete -f tests/k8s/deploy.yaml
    runtimeclass.node.k8s.io "youki" deleted
    deployment.apps "nginx-deployment" deleted
    ```

3. Returning to the first command executed, the system calls youki issued are caught and logged.
    
    ```console
    $ just hack-bpftrace
    BPFTRACE_STRLEN=120 ./hack/debug.bt
    Attaching 13 probes...
    Tracing Youki syscalls... Hit Ctrl-C to end.
    TIME                 COMMAND PID      EVENT     CONTENT
    207033348942           youki 13743    open      errno=2, fd=-1, file=/opt/containerd/lib/glibc-hwcaps/x86-64-v3/libc.so.6
    ...
    207035462044           youki 13743    open      errno=0, fd=3, file=/proc/self/exe
    207035478523           youki 13743    write     fd=4, ELF
    207066996623               4 13743    open      errno=2, fd=-1, file=/opt/containerd/lib/glibc-hwcaps/x86-64-v3/libc.so.6
    ...
    207070130175               4 13743    clone3
    207070418829 youki:[1:INTER] 13747    write     fd=4, {"timestamp":"2023-09-24T10:47:07.427846Z","level":"INFO","message":"cgroup manager V2 will be used","target":"libcgrou
    ...
    207084948440 youki:[1:INTER] 13747    clone3
    207085058811 youki:[1:INTER] 13747    write     fd=4, {"timestamp":"2023-09-24T10:47:07.442502Z","level":"DEBUG","message":"sending init pid (Pid(1305))","target":"libcontai
    207085343170  youki:[2:INIT] 13750    write     fd=4, {"timestamp":"2023-09-24T10:47:07.442746Z","level":"DEBUG","message":"unshare or setns: LinuxNamespace { typ: Uts, path
    ...
    207088256843  youki:[2:INIT] 13750    pivt_root new_root=/run/containerd/io.containerd.runtime.v2.task/k8s.io/0fea8cf5f8d1619a35ca67fd6fa73d8d7c8fc70ac2ed43ee2ac2f8610bb938f6/r, put_old=/run/containerd/io.containerd.runtime.v2.task/k8s.io/0fea8cf5f8d1619a35ca67fd6fa73d8d7c8fc70ac2ed43ee2ac2f8610bb938f6/r
    ...
    207097207551  youki:[2:INIT] 13750    write     fd=4, {"timestamp":"2023-09-24T10:47:07.454645Z","level":"DEBUG","message":"found executable in executor","executable":"\"/pa
    ...
    207139391811  youki:[2:INIT] 13750    write     fd=4, {"timestamp":"2023-09-24T10:47:07.496815Z","level":"DEBUG","message":"received: start container","target":"libcontainer
    207139423243  youki:[2:INIT] 13750    write     fd=4, {"timestamp":"2023-09-24T10:47:07.496868Z","level":"DEBUG","message":"executing workload with default handler","target"

    ```
