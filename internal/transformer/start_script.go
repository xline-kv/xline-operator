package transformer

const (
	DataDir          = "/usr/local/xline/data-dir"
	XlineStartScript = `#!/bin/bash

function process_cmd_args() {
	local envs=("${!1}")
	is_bool=$2
	args=""
	for arg_name in "${envs[@]}"; do
		if [ -n "${!arg_name}" ]; then
			arg=$(echo "-$arg_name" | sed 's/\([A-Z]\)/-\L\1/g')
			if [ "$is_bool" = true ]; then
				args="${args} ${arg} "
			else
				args="${args} ${arg} ${!arg_name} "
			fi
		fi
	done
	echo $args
}

function resolve_domain() {
	domain=$1
	elapseTime=0
	period=1
	threshold=30
	while true; do
		sleep ${period}
		elapseTime=$(( elapseTime+period ))

		if [[ ${elapseTime} -ge ${threshold} ]]
		then
			echo "waiting for xline cluster ready timeout" >&2
			exit 1
		fi

		if nslookup ${domain} 2>/dev/null
		then
			echo "nslookup domain ${domain}.svc success"
			break
		else
			echo "nslookup domain ${domain} failed" >&2
		fi
	done
}

bool_envs=("JaegerOffline" "JaegerOnline" "ClientUseBackoff")
number_envs=("RetryCount" "FollowerTimeoutTicks" "CandidateTimeoutTicks"
			"LogEntriesCap" "CmdWorkers" "CompactBatchSize" "Quota")
unit_envs=("HeartbeatInterval" "ServerWaitSyncedTimeout" "RetryTimeout"
			"RpcTimeout" "BatchTimeout" "ClientWaitSyncedTimeout"
			"ClientProposeTimeout" "ClientInitialRetryTimeout" "ClientMaxRetryTimeout"
			"GcInterval" "RangeRetryTimeout" "CompactTimeout" "SyncVictimsInterval"
			"WatchProgressNotifyInterval" "CompactSleepInterval" "BatchMaxSize")
enum_envs=("JaegerLevel" "LogRotate" "LogLevel")
file_envs=("JaegerOutputDir" "LogFile" "CurpDir" "DataDir" "AuthPrivateKey" "AuthPublicKey")

SERVICE_NAME=$(echo ${HOSTNAME} | sed 's/-[0-9]*$//')
domain="${HOSTNAME}.${SERVICE_NAME}.${NAMESPACE}.svc.cluster.local:2379"
resolve_domain ${domain}

cmd="/usr/local/bin/xline --name $HOSTNAME --members $MEMBERS --storage-engine rocksdb --data-dir /usr/local/xline/data-dir"

cmd="${cmd} \
		$(process_cmd_args bool_envs[@] true) \
		$(process_cmd_args number_envs[@] false) \
		$(process_cmd_args unit_envs[@] false) \
		$(process_cmd_args enum_envs[@] false) \
		$(process_cmd_args file_envs[@] false)"

RUST_LOG=${LogLevel} exec $cmd
`
)
