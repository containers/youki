# -*- mode: ruby -*-
# vi: set ft=ruby :

Vagrant.configure("2") do |config|
    config.vm.box = "fedora/33-cloud-base"
    config.vm.provider :virtualbox do |v|
      v.memory = 2048
      v.cpus = 2
    end
    config.vm.provider :libvirt do |v|
      v.memory = 2048
      v.cpus = 2
    end

    config.vm.synced_folder '.', '/vagrant', disabled: true

    config.vm.provision "shell", inline: <<-SHELL
      set -e -u -o pipefail
      yum install -y git gcc docker
      grubby --update-kernel=ALL --args="systemd.unified_cgroup_hierarchy=0"
      service docker start
    SHELL

    config.vm.provision "shell", privileged: false, inline: <<-SHELL
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      echo "export PATH=$PATH:$HOME/.cargo/bin" >> ~/.bashrc

      git clone https://github.com/containers/youki
    SHELL
  end
