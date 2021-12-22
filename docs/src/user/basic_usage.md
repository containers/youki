# Basic Usage

This section will explain how to use youki as a low-level container runtime with some other high-level container runtime such as docker.

Youki can also be used with other runtimes such as podman, but for this section, we will use docker as an example.

#### Using youki with a high-level runtime

This explains how to use youki as a lowe level runtime along with some high-level runtime. For this example we use Docker.

1. If you have the docker daemon running (which you probably have if you have installed docker), first you will need to stop it. For example, if you use systemd as your init system, you can use `systemctl stop docker`, with root permissions.
2. Start the docker daemon with you as the runtime :

   ```console
   dockerd --experimental --add-runtime="youki=$(pwd)/youki" # run in the root directory
   ```

   This will start the docker daemon with youki as the low-level runtime. This will keep the terminal busy, so you can start it as a background process or open a new terminal for next steps.

   In case you get an error message such as :

   ```
   failed to start daemon: pid file found, ensure docker is not running or delete /var/run/docker.pid
   ```

   This means you docker daemon is already running, and you need to stop is as explained in step 1.

3. Now you can use docker normally, and give youki as the runtime in the arguments :
   ```console
   docker run -it --rm --runtime youki busybox   # run a container
   ```
4. After you are done, you can stop the docker daemon process that was started in step 2, and restart the normal docker daemon, by using your init system, for example using `systemctl start docker` with root permissions, for systems using systemd.

#### Using Youki Standalone

You can use youki directly without a higher level runtime, but it will be tedious. This example shows how to create and run a container using only youki.

1. run

   ```console
   mkdir -p tutorial/rootfs
   ```

   This will create a directory which will be used as the root directory for the container.

2. run
   ```console
   cd tutorial
   docker export $(docker create busybox) | tar -C rootfs -xvf -
   ```
   This will export the basic file system structure needed to run the container. You can manually create all the directories, but it will be tedious.
3. run
   ```console
   ../youki spec
   ```
   This will generate the config.json file needed to setup the permissions and configuration for the container process.
   You can manually edit the file to customize the behavior of the container process. For example, you can edit the process.args to specify command to run in the process :
   ```json
   "process": {
   ...
   "args": [
     "sleep", "30"
   ],
   ...
    }
   ```
   Then go back to the root directory :
   ```console
   cd ..
   ```
4. After this you can use youki to :
   - create the container
   ```console
    # create a container with name `tutorial_container`
    sudo ./youki create -b tutorial tutorial_container
   ```
   - get the state of the container
   ```console
    # you can see the state the container is `created`
    sudo ./youki state tutorial_container
   ```
   - start the container
   ```console
    # start the container
    sudo ./youki start tutorial_container
   ```
   - list all the containers
   ```console
   # will show the list of containers, the container is `running`
   sudo ./youki list
   ```
   - delete the specific container
   ```console
   # delete the container
   sudo ./youki delete tutorial_container
   ```
5. The above step created the containers with root permissions, but youki can also create rootless containers,which does not need root permissions. for that, after exporting the rootfs from docker, when creating spec, use `--rootless` flag :
   ```console
   ../youki spec --rootless
   ```
   This will generate the spec needed for rootless containers.
   After this the steps are same, except you can run them without sudo and root access :
   ```console
   cd ..
   sudo ./youki create -b tutorial rootless_container
   sudo ./youki state rootless_container
   sudo ./youki start rootless_container
   sudo ./youki list
   sudo ./youki delete rootless_container
   ```
