${__E2E_TESTENV__:=false} && return 0 || __E2E_TESTENV__=true

source "$(dirname "${BASH_SOURCE[0]}")/k8s/kind.sh"
source "$(dirname "${BASH_SOURCE[0]}")/util/util.sh"
source "$(dirname "${BASH_SOURCE[0]}")/../common/common.sh"

function testenv::k8s::create() {
  testenv::k8s::kind::create
}

function testenv::k8s::delete() {
  testenv::k8s::kind::export
  testenv::k8s::kind::delete
}

function testenv::k8s::load_images() {
  # xline image
  xline_image="${XLINE_IMAGE:-datenlord/xline:latest}"
  docker pull "$xline_image" >/dev/null
  testenv::k8s::kind::load_image "$xline_image"
  # xline operator image, this needs to be built first
  testenv::k8s::kind::load_image datenlord/xline-operator:latest
  # etcdctl image
  docker pull gcr.io/etcd-development/etcd:v3.5.5 >/dev/null
  testenv::k8s::kind::load_image gcr.io/etcd-development/etcd:v3.5.5
}
