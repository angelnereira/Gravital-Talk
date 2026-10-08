// This is a generated file - do not edit.
//
// Generated from gravital/v1/pairing.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:core' as $core;

import 'package:protobuf/protobuf.dart' as $pb;

class PairingResult_Status extends $pb.ProtobufEnum {
  static const PairingResult_Status STATUS_UNSPECIFIED =
      PairingResult_Status._(0, _omitEnumNames ? '' : 'STATUS_UNSPECIFIED');
  static const PairingResult_Status SUCCESS =
      PairingResult_Status._(1, _omitEnumNames ? '' : 'SUCCESS');
  static const PairingResult_Status REJECTED =
      PairingResult_Status._(2, _omitEnumNames ? '' : 'REJECTED');
  static const PairingResult_Status TIMEOUT =
      PairingResult_Status._(3, _omitEnumNames ? '' : 'TIMEOUT');
  static const PairingResult_Status UNREACHABLE =
      PairingResult_Status._(4, _omitEnumNames ? '' : 'UNREACHABLE');
  static const PairingResult_Status AUTH_FAILED =
      PairingResult_Status._(5, _omitEnumNames ? '' : 'AUTH_FAILED');

  static const $core.List<PairingResult_Status> values = <PairingResult_Status>[
    STATUS_UNSPECIFIED,
    SUCCESS,
    REJECTED,
    TIMEOUT,
    UNREACHABLE,
    AUTH_FAILED,
  ];

  static final $core.List<PairingResult_Status?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 5);
  static PairingResult_Status? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const PairingResult_Status._(super.value, super.name);
}

const $core.bool _omitEnumNames =
    $core.bool.fromEnvironment('protobuf.omit_enum_names');
