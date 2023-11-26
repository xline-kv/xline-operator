${__E2E_TESTENV_MINIKUBE__:=false} && return 0 || __E2E_TESTENV_MINIKUBE__=true

function testenv::k8s::minikube::_cluster_status() {
  # return "Running", "Stopped"  or nothing
  minikube status | grep apiserver | awk -F : '{print $2}' | tr -d '[:space:]'
}

# ENVIRONMENT VARIABLES:
#   KIND_CLUSTER_IMAGE (optional): kind cluster image, default to _DEFAULT_KIND_IMAGE
function testenv::k8s::minikube::start() {
  KUBEVERSION="${KUBEVERSION:-v1.23.3}"
  status=$(testenv::k8s::minikube::_cluster_status)
  if [ "$status" == "Running" ]; then
    log::warn "minikube cluster already starts, skip strating"
  else
    log::info "Starting a minikube cluster ..."
    if ! minikube start --kubernetes-version=${KUBEVERSION} --image-mirror-country='cn' --driver docker --image-repository=registry.cn-hangzhou.aliyuncs.com/google_containers; then
      log::fatal "Failed to start a minikube cluster"
    fi
  fi
}

function testenv::k8s::minikube::stop() {
  status=$(testenv::k8s::minikube::_cluster_status)
  if [ "$status" == "Running" ]; then
    log::info "Stopping minikube cluster"
    if ! minikube delete; then
      log::fatal "Failed to stop minikube cluster"
    fi
  else
    log::warn "minikube cluster does not run, skip stopping"
  fi
}

function testenv::util::etcdctl() {
  # shellcheck disable=SC2034
  local KUBECTL_NAMESPACE="${_UTIL_NAMESPACE}"

  # retry to avoid mysterious "Error from server: error dialing backend: EOF" error
  for ((i = 0; i < ${RETRY_TIMES:-10}; i++)); do
    if output=$(k8s::kubectl exec -i etcdctl -- env ETCDCTL_API=3 etcdctl $@ 2>&1); then
      echo -e "$output"
      return
    fi
    sleep "${RETRY_INTERVAL:-3}"
  done
}
