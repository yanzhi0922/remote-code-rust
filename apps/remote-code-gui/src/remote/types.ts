export interface RemoteListResponse<T> {
  items: T[];
  latest_sequence?: number | null;
}

export type RemoteDeviceKind = 'runner' | 'browser' | 'cli';

export interface RemoteControlPlaneHealth {
  ok: boolean;
  service: string;
  phase: string;
  runner_count: number;
  available_runner_count: number;
  session_count: number;
  artifact_count: number;
  queued_runner_command_count: number;
  auth_required: boolean;
  bootstrap_secret_configured: boolean;
  owner_claimed: boolean;
  device_count: number;
}

export interface RemoteTrustedDeviceRecord {
  device_id: string;
  name: string;
  kind: RemoteDeviceKind;
  owner: boolean;
  created_by_device_id: string | null;
  created_at: string;
  last_seen_at: string;
}

export interface RemoteBootstrapClaimResponse {
  device: RemoteTrustedDeviceRecord;
  access_token: string;
  refresh_token: string;
}

export interface RemotePairingOfferCreateResponse {
  offer_id: string;
  device_name: string;
  device_kind: RemoteDeviceKind;
  created_at: string;
  expires_at: string;
  pairing_secret: string;
  pairing_url: string | null;
}

export interface RemotePairingAcceptResponse {
  device: RemoteTrustedDeviceRecord;
  access_token: string;
  refresh_token: string;
}

export type RemoteSessionState =
  | 'pending'
  | 'assigned'
  | 'running'
  | 'waiting_approval'
  | 'completed'
  | 'failed'
  | 'cancelled';

export interface RemoteSessionRecord {
  session_id: string;
  workspace_id: string;
  owner_runner_id: string | null;
  owner_runner_available?: boolean;
  owner_runner_state?: RemoteRunnerState | null;
  owner_runner_last_seen_at?: string | null;
  /** Direct-connect URL for the runner hosting this session. When present,
   *  the GUI can stream events and send commands directly to the runner. */
  owner_runner_public_base_url?: string | null;
  state: RemoteSessionState;
  metadata: Record<string, string>;
  created_at: string;
  updated_at: string;
}

export type RemoteApprovalState = 'pending' | 'approved' | 'denied' | 'cancelled';
export type RemoteApprovalDecision = Exclude<RemoteApprovalState, 'pending'>;

export interface RemoteApprovalRecord {
  approval_id: string;
  session_id: string;
  runner_id: string;
  state: RemoteApprovalState;
  title: string;
  description: string;
  metadata: Record<string, string>;
  created_at: string;
  updated_at: string;
  responded_at: string | null;
  responder: string | null;
  note: string | null;
}

export interface RemoteArtifactRecord {
  artifact_id: string;
  session_id: string;
  runner_id: string | null;
  name: string;
  file_name: string;
  media_type: string;
  size_bytes: number;
  metadata: Record<string, string>;
  created_at: string;
}

export type RemoteMessageRole = 'assistant' | 'user' | 'system';
export type RemoteDaemonPresenceState = 'online' | 'offline' | 'reconnecting';
export type RemoteRunnerState =
  | 'starting'
  | 'idle'
  | 'busy'
  | 'draining'
  | 'unhealthy'
  | 'offline';

export type RemoteTimelineEventDetail =
  | {
      kind: 'runner_registered';
      lease_ttl_secs: number;
      workspace_ids: string[];
      state: RemoteRunnerState;
    }
  | {
      kind: 'runner_heartbeat';
      state: RemoteRunnerState;
      active_sessions: number;
      queued_sessions: number;
      reported_at: string;
    }
  | {
      kind: 'session_created';
      workspace_id: string;
      owner_runner_id: string | null;
      state: RemoteSessionState;
    }
  | {
      kind: 'session_state_changed';
      previous_state: RemoteSessionState;
      state: RemoteSessionState;
    }
  | {
      kind: 'approval_requested';
      approval_id: string;
      title: string;
      state: RemoteApprovalState;
    }
  | {
      kind: 'approval_resolved';
      approval_id: string;
      state: RemoteApprovalState;
      responder: string | null;
    }
  | {
      kind: 'artifact_created';
      artifact_id: string;
      name: string;
      file_name: string;
      media_type: string;
      size_bytes: number;
    }
  | {
      kind: 'message_delta';
      role: RemoteMessageRole;
      delta: string;
      message_id?: string | null;
    }
  | {
      kind: 'message_committed';
      role: RemoteMessageRole;
      text: string;
      message_id?: string | null;
    }
  | {
      kind: 'tool_started';
      tool_call_id: string;
      tool_name: string;
    }
  | {
      kind: 'tool_progress';
      tool_call_id?: string | null;
      tool_name?: string | null;
      delta?: string | null;
      elapsed_time_seconds?: number | null;
    }
  | {
      kind: 'tool_finished';
      tool_call_id: string;
      tool_name: string;
      is_error: boolean;
      summary?: string | null;
    }
  | {
      kind: 'artifact_manifest';
      artifact_ids: string[];
    }
  | {
      kind: 'runtime_error';
      message: string;
    }
  | {
      kind: 'daemon_presence_changed';
      state: RemoteDaemonPresenceState;
    }
  | {
      kind: 'subtask_started';
      task_id: string;
      parent_task_id: string | null;
      description: string;
      depth: number;
    }
  | {
      kind: 'subtask_progress';
      task_id: string;
      status: string;
      summary: string;
    }
  | {
      kind: 'subtask_completed';
      task_id: string;
      status: string;
      summary: string;
      turns_used: number | null;
    }
  | {
      kind: 'batch_progress';
      total: number;
      completed: number;
      running: number;
    }
  | {
      kind: 'context_usage';
      estimated_tokens: number;
      max_input_tokens: number;
      threshold_tokens: number;
      ratio: number;
    }
  | {
      kind: 'context_overflow';
      estimated_tokens: number;
      max_input_tokens: number;
      threshold_tokens: number;
      ratio: number;
    }
  | {
      kind: 'context_compacted';
      entries_removed: number;
      usage_ratio: number;
    };

export interface RemoteTimelineEvent {
  sequence: number;
  recorded_at: string;
  runner_id: string | null;
  session_id: string | null;
  detail: RemoteTimelineEventDetail;
}

export interface RemoteCommandResponse {
  session_id: string;
  accepted: boolean;
  message: string;
}

// ---------------------------------------------------------------------------
// Push token registration (mobile devices)
// ---------------------------------------------------------------------------

export type RemotePushPlatform = 'apns' | 'fcm';

export interface RemotePushTokenRegistrationRequest {
  push_token: string;
  platform?: RemotePushPlatform;
}

export interface RemotePushTokenRegistrationResponse {
  registered: boolean;
}