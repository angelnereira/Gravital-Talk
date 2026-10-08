// This is a generated file - do not edit.
//
// Generated from gravital/v1/server_control.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:core' as $core;

import 'package:protobuf/protobuf.dart' as $pb;

class RoomEvent_EventType extends $pb.ProtobufEnum {
  static const RoomEvent_EventType EVENT_UNSPECIFIED =
      RoomEvent_EventType._(0, _omitEnumNames ? '' : 'EVENT_UNSPECIFIED');
  static const RoomEvent_EventType PEER_JOINED =
      RoomEvent_EventType._(1, _omitEnumNames ? '' : 'PEER_JOINED');
  static const RoomEvent_EventType PEER_LEFT =
      RoomEvent_EventType._(2, _omitEnumNames ? '' : 'PEER_LEFT');
  static const RoomEvent_EventType FLOOR_GRANTED =
      RoomEvent_EventType._(3, _omitEnumNames ? '' : 'FLOOR_GRANTED');
  static const RoomEvent_EventType FLOOR_RELEASED =
      RoomEvent_EventType._(4, _omitEnumNames ? '' : 'FLOOR_RELEASED');
  static const RoomEvent_EventType ROOM_CLOSED =
      RoomEvent_EventType._(5, _omitEnumNames ? '' : 'ROOM_CLOSED');

  static const $core.List<RoomEvent_EventType> values = <RoomEvent_EventType>[
    EVENT_UNSPECIFIED,
    PEER_JOINED,
    PEER_LEFT,
    FLOOR_GRANTED,
    FLOOR_RELEASED,
    ROOM_CLOSED,
  ];

  static final $core.List<RoomEvent_EventType?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 5);
  static RoomEvent_EventType? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const RoomEvent_EventType._(super.value, super.name);
}

const $core.bool _omitEnumNames =
    $core.bool.fromEnvironment('protobuf.omit_enum_names');
