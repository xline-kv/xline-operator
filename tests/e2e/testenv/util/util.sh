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
  echo -e "kubectl exec -n util -i etcdctl -- env ETCDCTL_API=3 etcdctl --endpoints=$1"
}

function testenv::util::run_with_expect() {
  cmd="$1"
  expect=$(echo -e ${2})
  # retry to avoid mysterious "Error from server: error dialing backend: EOF" error
  for ((k = 0; k < ${RETRY_TIMES:-10}; k++)); do
    output=$(eval ${cmd} 2>&1)
    if [[ $output == *"timed out"* || $output == *"Request timeout"* || $output == *"context deadline exceeded"* ]]; then
      sleep "${RETRY_INTERVAL:-3}"
    elif [ "${output//$'\r'/}" == "$expect" ]; then
      log::info "command $cmd run success"
      return 0
    else
      log::error "command $cmd run failed"
      log::error "expect: $expect"
      log::error "got: $output"
      return 1
    fi
  done
}

# run a command with expect output, based on key word match
# args:
#   $1: command to run
#   $2: key word to match
function testenv::util::run_with_match() {
  cmd="$1"
  expect=$(echo -e ${2})
  # retry to avoid mysterious "Error from server: error dialing backend: EOF" error
  for ((n = 0; n < ${RETRY_TIMES:-10}; n++)); do
    output=$(eval ${cmd} 2>&1)
    if [[ $output == *"timed out"* || $output == *"Request timeout"* || $output == *"context deadline exceeded"* ]]; then
      sleep "${RETRY_INTERVAL:-3}"
    elif echo "${output}" | grep -q "${expect}"; then
      log::info "command $cmd run success"
      return 0
    else
      log::error "command $cmd run failed"
      log::error "expect: $expect"
      log::error "got: $output"
      return 1
    fi
  done
}
