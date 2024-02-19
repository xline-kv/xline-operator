${__E2E_TESTENV__:=false} && return 0 || __E2E_TESTENV__=true

source "${E2E_TEST_DIR}/testenv/k8s/kind.sh"
source "${E2E_TEST_DIR}/testenv/util/util.sh"
source "${E2E_TEST_DIR}/common/common.sh"

function testenv::k8s::create() {
  testenv::k8s::kind::create
}

function testenv::k8s::delete() {
  testenv::k8s::kind::export
  testenv::k8s::kind::delete
}

function testenv::k8s::load_images() {
  log::info "Loading images"
  # xline operator image
  pushd ${CODE_BASE_DIR}
  IMG=${OPERATOR_IMG} make docker-build 2>/dev/null
  popd
  testenv::k8s::kind::load_image "$OPERATOR_IMG"

  remote_images=("phoenix500526/xline:v0.7.0" "ghcr.io/xline-kv/etcdctl:v3.5.9" "quay.io/brancz/kube-rbac-proxy:v0.15.0")
  for img in "${remote_images[@]}"; do
    docker pull "$img" 2>/dev/null
    testenv::k8s::kind::load_image "$img"
  done
}
