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
  # xline image
  log::info "Loading images"
  xline_image="phoenix500526/xline:v0.6.1"
  docker pull "$xline_image" 2>/dev/null
  testenv::k8s::kind::load_image "$xline_image"
  # etcdctl image
  etcdctl_image="ghcr.io/xline-kv/etcdctl:v3.5.9"
  docker pull "$etcdctl_image" 2>/dev/null
  testenv::k8s::kind::load_image "$etcdctl_image"
}
