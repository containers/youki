# Migration Guide

This contains information for migrating library versions.

## V0.1.0 -> v0.2.0

### libcontainer

- The `Rootless` struct has been re-named as `UserNamespaceConfig` , `RootlessIDMapper` has been re-named to `UserNamespaceIDMapper` , and correspondingly the `RootlessError` has been re-named to `UserNamespaceError` . This is due to the fact that the structure was to be used for containers when a new user namespace is to be created, and that is not strictly only for rootless uses. Accordingly, the fields of various structs has been updated to reflect this change :
  - rootless (module name) -> user_ns
  - Rootless::rootless_id_mapper -> UserNamespaceConfig::id_mapper
  - LibcontainerError::Rootless -> LibcontainerError::UserNamespace
  - ContainerBuilderImpl::rootless -> ContainerBuilderImpl::user_ns_config
  - ContainerArgs::rootless -> ContainerArgs::user_ns_config

- Changes that will occur for properly running in rootless mode : TODO (@YJDoc2)

- Executor now contains 2 methods for implementation. We introduce a `validate` step in addition to execute. The `validate` should validate the input OCI spec. The step runs after all the namespaces are entered and rootfs is pivoted.

- Executor is now composible instead of an array of executor. To implement multiple executor, create a new executor that runs all the executor. The users are now in control of how multiple executor are run.
