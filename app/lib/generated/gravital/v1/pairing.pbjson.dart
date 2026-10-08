// This is a generated file - do not edit.
//
// Generated from gravital/v1/pairing.proto.

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

@$core.Deprecated('Use pairingOfferDescriptor instead')
const PairingOffer$json = {
  '1': 'PairingOffer',
  '2': [
    {'1': 'uri', '3': 1, '4': 1, '5': 9, '10': 'uri'},
    {'1': 'device_name', '3': 2, '4': 1, '5': 9, '10': 'deviceName'},
    {'1': 'session_id', '3': 3, '4': 1, '5': 13, '10': 'sessionId'},
    {
      '1': 'generated_at_unix_ms',
      '3': 4,
      '4': 1,
      '5': 3,
      '10': 'generatedAtUnixMs'
    },
  ],
};

/// Descriptor for `PairingOffer`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pairingOfferDescriptor = $convert.base64Decode(
    'CgxQYWlyaW5nT2ZmZXISEAoDdXJpGAEgASgJUgN1cmkSHwoLZGV2aWNlX25hbWUYAiABKAlSCm'
    'RldmljZU5hbWUSHQoKc2Vzc2lvbl9pZBgDIAEoDVIJc2Vzc2lvbklkEi8KFGdlbmVyYXRlZF9h'
    'dF91bml4X21zGAQgASgDUhFnZW5lcmF0ZWRBdFVuaXhNcw==');

@$core.Deprecated('Use pairingAcceptDescriptor instead')
const PairingAccept$json = {
  '1': 'PairingAccept',
  '2': [
    {'1': 'accepted', '3': 1, '4': 1, '5': 8, '10': 'accepted'},
    {'1': 'reject_reason', '3': 2, '4': 1, '5': 9, '10': 'rejectReason'},
    {'1': 'fallback_relay', '3': 3, '4': 1, '5': 9, '10': 'fallbackRelay'},
  ],
};

/// Descriptor for `PairingAccept`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pairingAcceptDescriptor = $convert.base64Decode(
    'Cg1QYWlyaW5nQWNjZXB0EhoKCGFjY2VwdGVkGAEgASgIUghhY2NlcHRlZBIjCg1yZWplY3Rfcm'
    'Vhc29uGAIgASgJUgxyZWplY3RSZWFzb24SJQoOZmFsbGJhY2tfcmVsYXkYAyABKAlSDWZhbGxi'
    'YWNrUmVsYXk=');

@$core.Deprecated('Use pairingResultDescriptor instead')
const PairingResult$json = {
  '1': 'PairingResult',
  '2': [
    {
      '1': 'status',
      '3': 1,
      '4': 1,
      '5': 14,
      '6': '.gravital.v1.PairingResult.Status',
      '10': 'status'
    },
    {
      '1': 'negotiated_session_id',
      '3': 2,
      '4': 1,
      '5': 13,
      '10': 'negotiatedSessionId'
    },
    {'1': 'error_detail', '3': 3, '4': 1, '5': 9, '10': 'errorDetail'},
  ],
  '4': [PairingResult_Status$json],
};

@$core.Deprecated('Use pairingResultDescriptor instead')
const PairingResult_Status$json = {
  '1': 'Status',
  '2': [
    {'1': 'STATUS_UNSPECIFIED', '2': 0},
    {'1': 'SUCCESS', '2': 1},
    {'1': 'REJECTED', '2': 2},
    {'1': 'TIMEOUT', '2': 3},
    {'1': 'UNREACHABLE', '2': 4},
    {'1': 'AUTH_FAILED', '2': 5},
  ],
};

/// Descriptor for `PairingResult`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pairingResultDescriptor = $convert.base64Decode(
    'Cg1QYWlyaW5nUmVzdWx0EjkKBnN0YXR1cxgBIAEoDjIhLmdyYXZpdGFsLnYxLlBhaXJpbmdSZX'
    'N1bHQuU3RhdHVzUgZzdGF0dXMSMgoVbmVnb3RpYXRlZF9zZXNzaW9uX2lkGAIgASgNUhNuZWdv'
    'dGlhdGVkU2Vzc2lvbklkEiEKDGVycm9yX2RldGFpbBgDIAEoCVILZXJyb3JEZXRhaWwiagoGU3'
    'RhdHVzEhYKElNUQVRVU19VTlNQRUNJRklFRBAAEgsKB1NVQ0NFU1MQARIMCghSRUpFQ1RFRBAC'
    'EgsKB1RJTUVPVVQQAxIPCgtVTlJFQUNIQUJMRRAEEg8KC0FVVEhfRkFJTEVEEAU=');
