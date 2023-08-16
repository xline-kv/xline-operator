${__E2E_TESTENV_KIND__:=false} && return 0 || __E2E_TESTENV_KIND__=true

_TEST_ENV_KIND_CLUSTER_NAME="e2e-kind"
_DEFAULT_KIND_IMAGE="kindest/node:v1.25.3@sha256:f52781bc0d7a19fb6c405c2af83abfeb311f130707a0e219175677e366cc45d1"

source "$(dirname "${BASH_SOURCE[0]}")/../../common/common.sh"

function testenv::k8s::kind::_cluster_exists() {
  kind get clusters -q | grep -w -q "${_TEST_ENV_KIND_CLUSTER_NAME}"
}

# ENVIRONMENT VARIABLES:
#   KIND_CLUSTER_IMAGE (optional): kind cluster image, default to _DEFAULT_KIND_IMAGE
function testenv::k8s::kind::create() {
  local kind_image="${KIND_CLUSTER_IMAGE:-${_DEFAULT_KIND_IMAGE}}"
  if ! testenv::k8s::kind::_cluster_exists; then
    log::info "Creating kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME}"
    if ! kind create cluster --name "${_TEST_ENV_KIND_CLUSTER_NAME}" --image "${kind_image}"; then
      log::fatal "Failed to create kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME}"
    fi
  else
    log::warn "Kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME} already exists, skip creating"
  fi
}

function testenv::k8s::kind::export() {
  if testenv::k8s::kind::_cluster_exists; then
    log::info "Exporting logs kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME}"
    kind export logs --name ${_TEST_ENV_KIND_CLUSTER_NAME} /tmp/xlineoperator/${_TEST_ENV_KIND_CLUSTER_NAME}
  else
    log::warn "Kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME} does not exist, skip export logs"
  fi
}

function testenv::k8s::kind::delete() {
  if testenv::k8s::kind::_cluster_exists; then
    log::info "Deleting kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME}"
    if ! kind delete cluster --name "${_TEST_ENV_KIND_CLUSTER_NAME}"; then
      log::fatal "Failed to delete kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME}"
    fi
  else
    log::warn "Kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME} does not exist, skip deleting"
  fi
}

function testenv::k8s::kind::load_image() {
  log::info "Loading local docker images:" "$@"
  if ! testenv::k8s::kind::_cluster_exists; then
    log::fatal "Kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME} does not exist, cannot load image"
  fi
  if kind load docker-image --name "${_TEST_ENV_KIND_CLUSTER_NAME}" "$@"; then
    log::info "Successfully loaded image into kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME}"
  else
    log::error "Failed to load image into kind cluster ${_TEST_ENV_KIND_CLUSTER_NAME}"
  fi
}
