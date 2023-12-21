${__E2E_CASES_CI__:=false} && return 0 || __E2E_CASES_CI__=true

source "$(dirname "${BASH_SOURCE[0]}")/../common/common.sh"
source "$(dirname "${BASH_SOURCE[0]}")/../testenv/testenv.sh"

_TEST_CI_CLUSTER_NAME="my-xline-cluster"
_TEST_CI_STS_NAME="$_TEST_CI_CLUSTER_NAME-sts"
_TEST_CI_SVC_NAME="$_TEST_CI_CLUSTER_NAME-svc"
_TEST_CI_NAMESPACE="default"
_TEST_CI_DNS_SUFFIX="svc.cluster.local"
_TEST_CI_XLINE_PORT="2379"
_TEST_CI_LOG_SYNC_TIMEOUT=60

function test::ci::_mk_endpoints() {
  local endpoints="${_TEST_CI_STS_NAME}-0.${_TEST_CI_SVC_NAME}.${_TEST_CI_NAMESPACE}.${_TEST_CI_DNS_SUFFIX}:${_TEST_CI_XLINE_PORT}"
  for ((i = 1; i < $1; i++)); do
    endpoints="${endpoints},${_TEST_CI_STS_NAME}-${i}.${_TEST_CI_SVC_NAME}.${_TEST_CI_NAMESPACE}.${_TEST_CI_DNS_SUFFIX}:${_TEST_CI_XLINE_PORT}"
  done
  echo "$endpoints"
}

function test::ci::_etcdctl_expect() {
  log::debug "run command: etcdctl --endpoints=$1 $2"
  got=$(testenv::util::etcdctl --endpoints="$1" "$2")
  expect=$(echo -e "$3")
  if [ "${got//$'\r'/}" == "$expect" ]; then
    log::info "command run success"
  else
    log::error "command run failed"
    log::error "expect: $expect"
    log::error "got: $got"
    return 1
  fi
}

function test::ci::_install_CRD() {
    KUBECTL="minikube kubectl --" make install
    if [ $? -eq 0 ]; then
        log::info "make install: create custom resource definition succeeded"
    else
        log::error "make install: create custom resource definition failed"
    fi
}

function test::ci::_uninstall_CRD() {
    KUBECTL="minikube kubectl --" make uninstall
    if [ $? -eq 0 ]; then
        log::info "make uninstall: remove custom resource definition succeeded"
    else
        log::error "make uninstall: remove custom resource definition failed"
    fi
}

function test::ci::wait_all_xline_pod_ready() {
  for ((i = 0; i < $1; i++)); do
    log::info "wait pod/${_TEST_CI_STS_NAME}-${i} to be ready"
    if ! k8s::kubectl wait --for=condition=Ready pod/${_TEST_CI_STS_NAME}-${i} --timeout=300s; then
      log::fatal "Failed to wait for util to be ready"
    fi
  done
}

function test::ci::_start() {
  log::info "starting controller"
  pushd $(dirname "${BASH_SOURCE[0]}")/../../../
  test::ci::_install_CRD
  KUBECTL="minikube kubectl --" make run  >/dev/null 2>&1 &
  log::info "controller started"
  popd
  log::info "starting xline cluster"
  k8s::kubectl apply -f "$(dirname "${BASH_SOURCE[0]}")/manifests/cluster.yml" >/dev/null 2>&1
  k8s::kubectl::wait_resource_creation sts $_TEST_CI_STS_NAME
}

function test::ci::_teardown() {
  log::info "stopping controller"
  pushd $(dirname "${BASH_SOURCE[0]}")/../../../
  test::ci::_uninstall_CRD
  controller_pid=$(ps aux | grep "[g]o run ./cmd/main.go" | awk '{print $2}')
  if [ -n "$controller_pid" ]; then
    kill -9 $controller_pid
  fi
}

function test::ci::_chaos() {
  size=$1
  iters=$2
  max_kill=$((size / 2))
  log::info "chaos: size=$size, iters=$iters, max_kill=$max_kill"
  for ((i = 0; i < iters; i++)); do
    log::info "chaos: iter=$i"
    endpoints=$(test::ci::_mk_endpoints size)
    test::ci::_etcdctl_expect "$endpoints" "put A $i" "OK" || return $?
    test::ci::_etcdctl_expect "$endpoints" "get A" "A\n$i" || return $?
    kill=$((RANDOM % max_kill + 1))
    log::info "chaos: kill=$kill"
    for ((j = 0; j < kill; j++)); do
      pod="${_TEST_CI_STS_NAME}-$((RANDOM % size))"
      log::info "chaos: kill pod=$pod"
      k8s::kubectl delete pod "$pod" --force --grace-period=0 2>/dev/null
    done
    test::ci::_etcdctl_expect "$endpoints" "put B $i" "OK" || return $?
    test::ci::_etcdctl_expect "$endpoints" "get B" "B\n$i" || return $?
    k8s::kubectl wait --for=jsonpath='{.status.readyReplicas}'="$size" sts/$_TEST_CI_CLUSTER_NAME --timeout=300s >/dev/null 2>&1
    log::info "wait for log synchronization" && sleep $_TEST_CI_LOG_SYNC_TIMEOUT
  done
}

function test::run::ci::basic_validation() {
  test::ci::_start
  test::ci::wait_all_xline_pod_ready 3
  endpoints=$(test::ci::_mk_endpoints 3)
  test::ci::_etcdctl_expect "$endpoints" "put A 1" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n1" || return $?
  endpoints=$(test::ci::_mk_endpoints 1)
  test::ci::_etcdctl_expect "$endpoints" "put A 2" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n2" || return $?
  test::ci::_teardown
}


function test::run::ci::basic_chaos() {
  test::ci::_start
  test::ci::_chaos 3 5 || return $?
  test::ci::_teardown
}
