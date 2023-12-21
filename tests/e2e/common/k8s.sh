${__E2E_COMMON_K8S__:=false} && return 0 || __E2E_COMMON_K8S__=true

# ENVIRONMENT VARIABLES:
#   KUBECTL: path to kubectl binary
#   KUBECTL_NAMESPACE: namespace to use for kubectl commands
#
# ARGUMENTS:
#   $@: arguments to pass to kubectl
function k8s::kubectl() {
  KUBECTL_NAMESPACE="${KUBECTL_NAMESPACE:-default}"
  local kubectl="${KUBECTL:-minikube kubectl --}"
  ${kubectl} -n "${KUBECTL_NAMESPACE}" "$@"
}

# ENVIRONMENT VARIABLES:
#   KUBECTL: path to kubectl binary
#   KUBECTL_NAMESPACE: namespace to use for kubectl commands
#
# ARGUMENTS:
#   $1: resource type
#   $2: resource name
function k8s::kubectl::resource_exist() {
  KUBECTL_NAMESPACE="${KUBECTL_NAMESPACE:-default}"
  k8s::kubectl get "$1" "$2" >/dev/null 2>&1
}

# ENVIRONMENT VARIABLES:
#   KUBECTL: path to kubectl binary
#   KUBECTL_NAMESPACE: namespace to use for kubectl commands
#
# ARGUMENTS:
#   $1: resource type
#   $2: resource name
#   $3: (optional) interval to check resource creation
function k8s::kubectl::wait_resource_creation() {
  interval="${3:-5}"
  KUBECTL_NAMESPACE="${KUBECTL_NAMESPACE:-default}"
  while true; do
    if k8s::kubectl::resource_exist "$1" "$2"; then
      break
    fi
    log::info "Waiting for $1/$2 ($KUBECTL_NAMESPACE) to be created"
    sleep "$interval"
  done
}
