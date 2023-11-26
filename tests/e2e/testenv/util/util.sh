${__E2E_TESTENV_UTIL__:=false} && return 0 || __E2E_TESTENV_UTIL__=true

_TEST_ENV_UTIL_PATH="$(dirname "${BASH_SOURCE[0]}")"
_UTIL_NAMESPACE="util"

source "${_TEST_ENV_UTIL_PATH}/../../common/common.sh"

function testenv::util::_relative_path() {
  echo "${_TEST_ENV_UTIL_PATH}/$1"
}

function testenv::util::_is_installed() {
  k8s::kubectl::resource_exist namespace "${_UTIL_NAMESPACE}"
}

function testenv::util::install() {
  if ! testenv::util::_is_installed; then
    log::info "Installing util"
    if ! k8s::kubectl create namespace "${_UTIL_NAMESPACE}"; then
      log::fatal "Failed to create namespace ${_UTIL_NAMESPACE}"
    fi
    # shellcheck disable=SC2034
    local KUBECTL_NAMESPACE="${_UTIL_NAMESPACE}"
    k8s::kubectl::wait_resource_creation serviceaccount default

    if ! k8s::kubectl apply -f "$(testenv::util::_relative_path manifests)"; then
      log::fatal "Failed to install util"
    fi
    if ! k8s::kubectl wait --for=condition=Ready pod/etcdctl --timeout=300s; then
      log::fatal "Failed to wait for util to be ready"
    fi
  else
    log::warn "Util already installed, skip installing"
  fi
}

function testenv::util::uninstall() {
  if testenv::util::_is_installed; then
    log::info "Uninstalling util"
    if ! k8s::kubectl delete namespace "${_UTIL_NAMESPACE}"; then
      log::fatal "Failed to delete namespace ${_UTIL_NAMESPACE}"
    fi
  else
    log::warn "Util not installed, skip uninstalling"
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
