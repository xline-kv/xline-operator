${__E2E_TESTENV__:=false} && return 0 || __E2E_TESTENV__=true

source "$(dirname "${BASH_SOURCE[0]}")/minikube.sh"
source "$(dirname "${BASH_SOURCE[0]}")/util/util.sh"
source "$(dirname "${BASH_SOURCE[0]}")/../common/common.sh"

function testenv::k8s::create() {
  testenv::k8s::minikube::start
}

function testenv::k8s::delete() {
  testenv::k8s::minikube::stop
}
