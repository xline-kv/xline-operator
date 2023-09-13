${__E2E_CASES_CI__:=false} && return 0 || __E2E_CASES_CI__=true

source "$(dirname "${BASH_SOURCE[0]}")/../common/common.sh"
source "$(dirname "${BASH_SOURCE[0]}")/../testenv/testenv.sh"

_TEST_CI_CLUSTER_NAME="my-xline-cluster"
_TEST_CI_OPERATOR_NAME="my-xline-operator"
_TEST_CI_DNS_SUFFIX="cluster.local"
_TEST_CI_NAMESPACE="default"
_TEST_CI_XLINE_PORT="2379"
_TEST_CI_LOG_SYNC_TIMEOUT=30
_TEST_CI_START_SIZE=3

function test::ci::_mk_endpoints() {
  local endpoints="${_TEST_CI_CLUSTER_NAME}-nodes-0.${_TEST_CI_CLUSTER_NAME}-svc.${_TEST_CI_NAMESPACE}.svc.${_TEST_CI_DNS_SUFFIX}:${_TEST_CI_XLINE_PORT}"
  for ((i = 1; i < $1; i++)); do
    endpoints="${endpoints},${_TEST_CI_CLUSTER_NAME}-nodes-${i}.${_TEST_CI_CLUSTER_NAME}-svc.${_TEST_CI_NAMESPACE}.svc.${_TEST_CI_DNS_SUFFIX}:${_TEST_CI_XLINE_PORT}"
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

function test::ci::_wait_for_cluster_ready() {
  size=$1
  scaled=${2-false}
  $scaled && k8s::kubectl wait --for=jsonpath='{.status.updatedReplicas}'="$size" sts "${_TEST_CI_CLUSTER_NAME}-nodes" --timeout=300s >/dev/null 2>&1
  k8s::kubectl wait --for=jsonpath='{.status.readyReplicas}'="$size" sts "${_TEST_CI_CLUSTER_NAME}-nodes" --timeout=300s >/dev/null 2>&1
}

function test::ci::_start() {
  log::info "starting cluster"
  k8s::kubectl create clusterrolebinding serviceaccount-cluster-admin --clusterrole=cluster-admin --serviceaccount=default:default 2>/dev/null || true
  k8s::kubectl apply -f "$(dirname "${BASH_SOURCE[0]}")/manifests/operators.yml" >/dev/null 2>&1
  k8s::kubectl wait --for=condition=available deployment/$_TEST_CI_OPERATOR_NAME --timeout=300s >/dev/null 2>&1
  k8s::kubectl::wait_resource_creation crd xlineclusters.xlineoperator.xline.cloud
  k8s::kubectl apply -f "$(dirname "${BASH_SOURCE[0]}")/manifests/cluster.yml" >/dev/null 2>&1
  k8s::kubectl::wait_resource_creation sts "${_TEST_CI_CLUSTER_NAME}-nodes"
  test::ci::_wait_for_cluster_ready $_TEST_CI_START_SIZE true
  log::info "cluster started"
}

function test::ci::_teardown() {
  if k8s::kubectl::resource_exist xc $_TEST_CI_CLUSTER_NAME || k8s::kubectl::resource_exist deployment $_TEST_CI_OPERATOR_NAME; then
    log::info "teardown cluster"
    k8s::kubectl delete -f "$(dirname "${BASH_SOURCE[0]}")/manifests/cluster.yml" 2>/dev/null || true
    k8s::kubectl delete -f "$(dirname "${BASH_SOURCE[0]}")/manifests/operators.yml" 2>/dev/null || true
  fi
}

function test::ci::_scale_cluster() {
  log::info "scaling cluster to $1"
  k8s::kubectl scale xc $_TEST_CI_CLUSTER_NAME --replicas="$1" >/dev/null 2>&1
  test::ci::_wait_for_cluster_ready "$1" true
  got=$(k8s::kubectl get xc $_TEST_CI_CLUSTER_NAME -o=jsonpath='{.spec.size}')
  if [ "$got" -ne "$1" ]; then
    echo "failed scale cluster"
    echo "expect size: $1"
    echo "got size: $got"
    return 1
  fi
  log::info "cluster scaled to $1"
}

function test::ci::_random_crash() {
  size=$1
  max_kill=$((size / 2))
  kill=$((RANDOM % max_kill + 1))
  log::info "random_crash_pods: size=$size, kill=$kill"
  for ((j = 0; j < kill; j++)); do
    pod="${_TEST_CI_CLUSTER_NAME}-nodes-$((RANDOM % size))"
    log::info "random_crash_pods: kill pod=$pod"
    k8s::kubectl delete pod "$pod" --force --grace-period=0 2>/dev/null
  done
  # check if it is ok
  endpoints=$(test::ci::_mk_endpoints "$size")
  test::ci::_etcdctl_expect "$endpoints" "put B 1" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get B" "B\n1" || return $?
  test::ci::_wait_for_cluster_ready "$size"
  log::info "wait for log synchronization" && sleep $_TEST_CI_LOG_SYNC_TIMEOUT
}

function test::run::ci::basic_validation() {
  test::ci::_teardown
  test::ci::_start

  endpoints=$(test::ci::_mk_endpoints 3)
  test::ci::_etcdctl_expect "$endpoints" "put A 1" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n1" || return $?
  endpoints=$(test::ci::_mk_endpoints 1)
  test::ci::_etcdctl_expect "$endpoints" "put A 2" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n2" || return $?
}

function test::run::ci::scale_validation() {
  test::ci::_teardown
  test::ci::_start

  test::ci::_scale_cluster 5 || return $?
  endpoints=$(test::ci::_mk_endpoints 5)
  test::ci::_etcdctl_expect "$endpoints" "put A 1" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n1" || return $?

  test::ci::_scale_cluster 3 || return $?
  log::info "wait for log synchronization" && sleep $_TEST_CI_LOG_SYNC_TIMEOUT
  endpoints=$(test::ci::_mk_endpoints 3)
  test::ci::_etcdctl_expect "$endpoints" "put A 2" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n2" || return $?
}

function test::run::ci::pod_crash_recovery() {
  test::ci::_teardown
  test::ci::_start

  test::ci::_random_crash 3
  endpoints=$(test::ci::_mk_endpoints 5)
  test::ci::_etcdctl_expect "$endpoints" "put A 1" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n1" || return $?
}
