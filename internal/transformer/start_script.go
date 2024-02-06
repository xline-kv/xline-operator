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

cmd="/usr/local/bin/xline --name $HOSTNAME --members $MEMBERS --storage-engine rocksdb --data-dir /usr/local/xline/data-dir"

cmd="${cmd} \
		$(process_cmd_args bool_envs[@] true) \
		$(process_cmd_args number_envs[@] false) \
		$(process_cmd_args unit_envs[@] false) \
		$(process_cmd_args enum_envs[@] false) \
		$(process_cmd_args file_envs[@] false)"

exec $cmd
`
)
