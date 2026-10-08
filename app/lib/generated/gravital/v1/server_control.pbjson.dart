// This is a generated file - do not edit.
//
// Generated from gravital/v1/server_control.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports
// ignore_for_file: unused_import

import 'dart:convert' as $convert;
import 'dart:core' as $core;
import 'dart:typed_data' as $typed_data;

@$core.Deprecated('Use roomDescriptor instead')
const Room$json = {
  '1': 'Room',
  '2': [
    {'1': 'code', '3': 1, '4': 1, '5': 9, '10': 'code'},
    {'1': 'session_id', '3': 2, '4': 1, '5': 13, '10': 'sessionId'},
    {'1': 'peer_count', '3': 3, '4': 1, '5': 5, '10': 'peerCount'},
    {
      '1': 'created_at_unix_ms',
      '3': 4,
      '4': 1,
      '5': 3,
      '10': 'createdAtUnixMs'
    },
  ],
};

/// Descriptor for `Room`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List roomDescriptor = $convert.base64Decode(
    'CgRSb29tEhIKBGNvZGUYASABKAlSBGNvZGUSHQoKc2Vzc2lvbl9pZBgCIAEoDVIJc2Vzc2lvbk'
    'lkEh0KCnBlZXJfY291bnQYAyABKAVSCXBlZXJDb3VudBIrChJjcmVhdGVkX2F0X3VuaXhfbXMY'
    'BCABKANSD2NyZWF0ZWRBdFVuaXhNcw==');

@$core.Deprecated('Use createRoomRequestDescriptor instead')
const CreateRoomRequest$json = {
  '1': 'CreateRoomRequest',
  '2': [
    {'1': 'session_id', '3': 1, '4': 1, '5': 13, '10': 'sessionId'},
    {'1': 'display_name', '3': 2, '4': 1, '5': 9, '10': 'displayName'},
    {'1': 'token', '3': 3, '4': 1, '5': 9, '10': 'token'},
  ],
};

/// Descriptor for `CreateRoomRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createRoomRequestDescriptor = $convert.base64Decode(
    'ChFDcmVhdGVSb29tUmVxdWVzdBIdCgpzZXNzaW9uX2lkGAEgASgNUglzZXNzaW9uSWQSIQoMZG'
    'lzcGxheV9uYW1lGAIgASgJUgtkaXNwbGF5TmFtZRIUCgV0b2tlbhgDIAEoCVIFdG9rZW4=');

@$core.Deprecated('Use createRoomResponseDescriptor instead')
const CreateRoomResponse$json = {
  '1': 'CreateRoomResponse',
  '2': [
    {
      '1': 'room',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.gravital.v1.Room',
      '10': 'room'
    },
  ],
};

/// Descriptor for `CreateRoomResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createRoomResponseDescriptor = $convert.base64Decode(
    'ChJDcmVhdGVSb29tUmVzcG9uc2USJQoEcm9vbRgBIAEoCzIRLmdyYXZpdGFsLnYxLlJvb21SBH'
    'Jvb20=');

@$core.Deprecated('Use getRoomRequestDescriptor instead')
const GetRoomRequest$json = {
  '1': 'GetRoomRequest',
  '2': [
    {'1': 'code', '3': 1, '4': 1, '5': 9, '10': 'code'},
    {'1': 'token', '3': 2, '4': 1, '5': 9, '10': 'token'},
  ],
};

/// Descriptor for `GetRoomRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List getRoomRequestDescriptor = $convert.base64Decode(
    'Cg5HZXRSb29tUmVxdWVzdBISCgRjb2RlGAEgASgJUgRjb2RlEhQKBXRva2VuGAIgASgJUgV0b2'
    'tlbg==');

@$core.Deprecated('Use getRoomResponseDescriptor instead')
const GetRoomResponse$json = {
  '1': 'GetRoomResponse',
  '2': [
    {
      '1': 'room',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.gravital.v1.Room',
      '10': 'room'
    },
  ],
};

/// Descriptor for `GetRoomResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List getRoomResponseDescriptor = $convert.base64Decode(
    'Cg9HZXRSb29tUmVzcG9uc2USJQoEcm9vbRgBIAEoCzIRLmdyYXZpdGFsLnYxLlJvb21SBHJvb2'
    '0=');

@$core.Deprecated('Use listRoomsRequestDescriptor instead')
const ListRoomsRequest$json = {
  '1': 'ListRoomsRequest',
};

/// Descriptor for `ListRoomsRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listRoomsRequestDescriptor =
    $convert.base64Decode('ChBMaXN0Um9vbXNSZXF1ZXN0');

@$core.Deprecated('Use listRoomsResponseDescriptor instead')
const ListRoomsResponse$json = {
  '1': 'ListRoomsResponse',
  '2': [
    {
      '1': 'rooms',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.gravital.v1.Room',
      '10': 'rooms'
    },
  ],
};

/// Descriptor for `ListRoomsResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listRoomsResponseDescriptor = $convert.base64Decode(
    'ChFMaXN0Um9vbXNSZXNwb25zZRInCgVyb29tcxgBIAMoCzIRLmdyYXZpdGFsLnYxLlJvb21SBX'
    'Jvb21z');

@$core.Deprecated('Use deleteRoomRequestDescriptor instead')
const DeleteRoomRequest$json = {
  '1': 'DeleteRoomRequest',
  '2': [
    {'1': 'code', '3': 1, '4': 1, '5': 9, '10': 'code'},
  ],
};

/// Descriptor for `DeleteRoomRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List deleteRoomRequestDescriptor = $convert
    .base64Decode('ChFEZWxldGVSb29tUmVxdWVzdBISCgRjb2RlGAEgASgJUgRjb2Rl');

@$core.Deprecated('Use deleteRoomResponseDescriptor instead')
const DeleteRoomResponse$json = {
  '1': 'DeleteRoomResponse',
  '2': [
    {'1': 'deleted', '3': 1, '4': 1, '5': 8, '10': 'deleted'},
  ],
};

/// Descriptor for `DeleteRoomResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List deleteRoomResponseDescriptor =
    $convert.base64Decode(
        'ChJEZWxldGVSb29tUmVzcG9uc2USGAoHZGVsZXRlZBgBIAEoCFIHZGVsZXRlZA==');

@$core.Deprecated('Use serverInfoRequestDescriptor instead')
const ServerInfoRequest$json = {
  '1': 'ServerInfoRequest',
};

/// Descriptor for `ServerInfoRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List serverInfoRequestDescriptor =
    $convert.base64Decode('ChFTZXJ2ZXJJbmZvUmVxdWVzdA==');

@$core.Deprecated('Use serverInfoResponseDescriptor instead')
const ServerInfoResponse$json = {
  '1': 'ServerInfoResponse',
  '2': [
    {'1': 'version', '3': 1, '4': 1, '5': 9, '10': 'version'},
    {'1': 'protocol_version', '3': 2, '4': 1, '5': 13, '10': 'protocolVersion'},
    {'1': 'max_sessions', '3': 3, '4': 1, '5': 5, '10': 'maxSessions'},
    {
      '1': 'max_peers_per_session',
      '3': 4,
      '4': 1,
      '5': 5,
      '10': 'maxPeersPerSession'
    },
    {'1': 'active_sessions', '3': 5, '4': 1, '5': 3, '10': 'activeSessions'},
    {'1': 'active_peers', '3': 6, '4': 1, '5': 3, '10': 'activePeers'},
    {'1': 'uptime_secs', '3': 7, '4': 1, '5': 9, '10': 'uptimeSecs'},
  ],
};

/// Descriptor for `ServerInfoResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List serverInfoResponseDescriptor = $convert.base64Decode(
    'ChJTZXJ2ZXJJbmZvUmVzcG9uc2USGAoHdmVyc2lvbhgBIAEoCVIHdmVyc2lvbhIpChBwcm90b2'
    'NvbF92ZXJzaW9uGAIgASgNUg9wcm90b2NvbFZlcnNpb24SIQoMbWF4X3Nlc3Npb25zGAMgASgF'
    'UgttYXhTZXNzaW9ucxIxChVtYXhfcGVlcnNfcGVyX3Nlc3Npb24YBCABKAVSEm1heFBlZXJzUG'
    'VyU2Vzc2lvbhInCg9hY3RpdmVfc2Vzc2lvbnMYBSABKANSDmFjdGl2ZVNlc3Npb25zEiEKDGFj'
    'dGl2ZV9wZWVycxgGIAEoA1ILYWN0aXZlUGVlcnMSHwoLdXB0aW1lX3NlY3MYByABKAlSCnVwdG'
    'ltZVNlY3M=');

@$core.Deprecated('Use serverHealthRequestDescriptor instead')
const ServerHealthRequest$json = {
  '1': 'ServerHealthRequest',
};

/// Descriptor for `ServerHealthRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List serverHealthRequestDescriptor =
    $convert.base64Decode('ChNTZXJ2ZXJIZWFsdGhSZXF1ZXN0');

@$core.Deprecated('Use serverHealthResponseDescriptor instead')
const ServerHealthResponse$json = {
  '1': 'ServerHealthResponse',
  '2': [
    {'1': 'status', '3': 1, '4': 1, '5': 9, '10': 'status'},
    {'1': 'packets_in', '3': 2, '4': 1, '5': 4, '10': 'packetsIn'},
    {'1': 'packets_out', '3': 3, '4': 1, '5': 4, '10': 'packetsOut'},
    {'1': 'bytes_in', '3': 4, '4': 1, '5': 4, '10': 'bytesIn'},
    {'1': 'bytes_out', '3': 5, '4': 1, '5': 4, '10': 'bytesOut'},
    {'1': 'dropped_invalid', '3': 6, '4': 1, '5': 4, '10': 'droppedInvalid'},
    {'1': 'active_sessions', '3': 7, '4': 1, '5': 3, '10': 'activeSessions'},
  ],
};

/// Descriptor for `ServerHealthResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List serverHealthResponseDescriptor = $convert.base64Decode(
    'ChRTZXJ2ZXJIZWFsdGhSZXNwb25zZRIWCgZzdGF0dXMYASABKAlSBnN0YXR1cxIdCgpwYWNrZX'
    'RzX2luGAIgASgEUglwYWNrZXRzSW4SHwoLcGFja2V0c19vdXQYAyABKARSCnBhY2tldHNPdXQS'
    'GQoIYnl0ZXNfaW4YBCABKARSB2J5dGVzSW4SGwoJYnl0ZXNfb3V0GAUgASgEUghieXRlc091dB'
    'InCg9kcm9wcGVkX2ludmFsaWQYBiABKARSDmRyb3BwZWRJbnZhbGlkEicKD2FjdGl2ZV9zZXNz'
    'aW9ucxgHIAEoA1IOYWN0aXZlU2Vzc2lvbnM=');

@$core.Deprecated('Use roomEventDescriptor instead')
const RoomEvent$json = {
  '1': 'RoomEvent',
  '2': [
    {
      '1': 'type',
      '3': 1,
      '4': 1,
      '5': 14,
      '6': '.gravital.v1.RoomEvent.EventType',
      '10': 'type'
    },
    {'1': 'room_code', '3': 2, '4': 1, '5': 9, '10': 'roomCode'},
    {'1': 'session_id', '3': 3, '4': 1, '5': 13, '10': 'sessionId'},
    {'1': 'at_unix_ms', '3': 4, '4': 1, '5': 3, '10': 'atUnixMs'},
    {'1': 'peer_ssrc', '3': 5, '4': 1, '5': 13, '10': 'peerSsrc'},
  ],
  '4': [RoomEvent_EventType$json],
};

@$core.Deprecated('Use roomEventDescriptor instead')
const RoomEvent_EventType$json = {
  '1': 'EventType',
  '2': [
    {'1': 'EVENT_UNSPECIFIED', '2': 0},
    {'1': 'PEER_JOINED', '2': 1},
    {'1': 'PEER_LEFT', '2': 2},
    {'1': 'FLOOR_GRANTED', '2': 3},
    {'1': 'FLOOR_RELEASED', '2': 4},
    {'1': 'ROOM_CLOSED', '2': 5},
  ],
};

/// Descriptor for `RoomEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List roomEventDescriptor = $convert.base64Decode(
    'CglSb29tRXZlbnQSNAoEdHlwZRgBIAEoDjIgLmdyYXZpdGFsLnYxLlJvb21FdmVudC5FdmVudF'
    'R5cGVSBHR5cGUSGwoJcm9vbV9jb2RlGAIgASgJUghyb29tQ29kZRIdCgpzZXNzaW9uX2lkGAMg'
    'ASgNUglzZXNzaW9uSWQSHAoKYXRfdW5peF9tcxgEIAEoA1IIYXRVbml4TXMSGwoJcGVlcl9zc3'
    'JjGAUgASgNUghwZWVyU3NyYyJ6CglFdmVudFR5cGUSFQoRRVZFTlRfVU5TUEVDSUZJRUQQABIP'
    'CgtQRUVSX0pPSU5FRBABEg0KCVBFRVJfTEVGVBACEhEKDUZMT09SX0dSQU5URUQQAxISCg5GTE'
    '9PUl9SRUxFQVNFRBAEEg8KC1JPT01fQ0xPU0VEEAU=');

@$core.Deprecated('Use watchRoomRequestDescriptor instead')
const WatchRoomRequest$json = {
  '1': 'WatchRoomRequest',
  '2': [
    {'1': 'room_code', '3': 1, '4': 1, '5': 9, '10': 'roomCode'},
  ],
};

/// Descriptor for `WatchRoomRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List watchRoomRequestDescriptor = $convert.base64Decode(
    'ChBXYXRjaFJvb21SZXF1ZXN0EhsKCXJvb21fY29kZRgBIAEoCVIIcm9vbUNvZGU=');
