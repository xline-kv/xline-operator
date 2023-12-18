${__E2E_COMMON_K8S__:=false} && return 0 || __E2E_COMMON_K8S__=true

# ENVIRONMENT VARIABLES:
#   KUBECTL: path to kubectl binary
#   KUBECTL_NAMESPACE: namespace to use for kubectl commands
#
# ARGUMENTS:
#   $@: arguments to pass to kubectl
function k8s::kubectl() {
  KUBECTL_NAMESPACE="${KUBECTL_NAMESPACE:-default}"
  local kubectl="${KUBECTL:-kubectl}"
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
  retry_limit="${4:-10}"
  retry_count=0

  while true; do
    if k8s::kubectl::resource_exist "$1" "$2"; then
      log::info "Resource $1/$2 ($KUBECTL_NAMESPACE) created"
      break
    fi

    if [ "$retry_count" -ge "$retry_limit" ]; then
      log::error "Exceeded retry limit for $1/$2 ($KUBECTL_NAMESPACE)"
      break
    fi

    retry_count=$((retry_count + 1))

    log::info "Waiting for $1/$2 ($KUBECTL_NAMESPACE) to be created"
    sleep "$interval"
  done
}
