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

import 'package:fixnum/fixnum.dart' as $fixnum;
import 'package:protobuf/protobuf.dart' as $pb;

import 'server_control.pbenum.dart';

export 'package:protobuf/protobuf.dart' show GeneratedMessageGenericExtensions;

export 'server_control.pbenum.dart';

/// Estado y metadatos de una sala.
class Room extends $pb.GeneratedMessage {
  factory Room({
    $core.String? code,
    $core.int? sessionId,
    $core.int? peerCount,
    $fixnum.Int64? createdAtUnixMs,
  }) {
    final result = Room._();
    if (code != null) result.code = code;
    if (sessionId != null) result.sessionId = sessionId;
    if (peerCount != null) result.peerCount = peerCount;
    if (createdAtUnixMs != null) result.createdAtUnixMs = createdAtUnixMs;
    return result;
  }

  Room._();

  factory Room.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Room()..mergeFromBuffer(data, registry);
  factory Room.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Room()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Room',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: Room.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'code')
    ..aI(2, _omitFieldNames ? '' : 'sessionId', fieldType: $pb.PbFieldType.OU3)
    ..aI(3, _omitFieldNames ? '' : 'peerCount')
    ..aInt64(4, _omitFieldNames ? '' : 'createdAtUnixMs')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Room clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Room copyWith(void Function(Room) updates) =>
      super.copyWith((message) => updates(message as Room)) as Room;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use Room() / Room.new instead')
  static Room create() => Room._();
  static $pb.GeneratedMessage $_createMessage() => Room._();
  @$core.override
  Room createEmptyInstance() => Room._();
  @$core.pragma('dart2js:noInline')
  static Room getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<Room>(Room.$_createMessage);
  static Room? _defaultInstance;

  /// Código legible comparte (XXXX-NNNN).
  @$pb.TagNumber(1)
  $core.String get code => $_getSZ(0);
  @$pb.TagNumber(1)
  set code($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasCode() => $_has(0);
  @$pb.TagNumber(1)
  void clearCode() => $_clearField(1);

  /// Id numérico de sesión: todos los peers de la sala lo comparten.
  @$pb.TagNumber(2)
  $core.int get sessionId => $_getIZ(1);
  @$pb.TagNumber(2)
  set sessionId($core.int value) => $_setUnsignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasSessionId() => $_has(1);
  @$pb.TagNumber(2)
  void clearSessionId() => $_clearField(2);

  /// Número de peers conectados.
  @$pb.TagNumber(3)
  $core.int get peerCount => $_getIZ(2);
  @$pb.TagNumber(3)
  set peerCount($core.int value) => $_setSignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasPeerCount() => $_has(2);
  @$pb.TagNumber(3)
  void clearPeerCount() => $_clearField(3);

  /// Timestamp de creación (unix ms; 0 = desconocido).
  @$pb.TagNumber(4)
  $fixnum.Int64 get createdAtUnixMs => $_getI64(3);
  @$pb.TagNumber(4)
  set createdAtUnixMs($fixnum.Int64 value) => $_setInt64(3, value);
  @$pb.TagNumber(4)
  $core.bool hasCreatedAtUnixMs() => $_has(3);
  @$pb.TagNumber(4)
  void clearCreatedAtUnixMs() => $_clearField(4);
}

/// Solicitud de creación de sala (host).
class CreateRoomRequest extends $pb.GeneratedMessage {
  factory CreateRoomRequest({
    $core.int? sessionId,
    $core.String? displayName,
  }) {
    final result = CreateRoomRequest._();
    if (sessionId != null) result.sessionId = sessionId;
    if (displayName != null) result.displayName = displayName;
    return result;
  }

  CreateRoomRequest._();

  factory CreateRoomRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CreateRoomRequest()..mergeFromBuffer(data, registry);
  factory CreateRoomRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CreateRoomRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CreateRoomRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: CreateRoomRequest.$_createMessage)
    ..aI(1, _omitFieldNames ? '' : 'sessionId', fieldType: $pb.PbFieldType.OU3)
    ..aOS(2, _omitFieldNames ? '' : 'displayName')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CreateRoomRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CreateRoomRequest copyWith(void Function(CreateRoomRequest) updates) =>
      super.copyWith((message) => updates(message as CreateRoomRequest))
          as CreateRoomRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use CreateRoomRequest() / CreateRoomRequest.new instead')
  static CreateRoomRequest create() => CreateRoomRequest._();
  static $pb.GeneratedMessage $_createMessage() => CreateRoomRequest._();
  @$core.override
  CreateRoomRequest createEmptyInstance() => CreateRoomRequest._();
  @$core.pragma('dart2js:noInline')
  static CreateRoomRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<CreateRoomRequest>(
          CreateRoomRequest.$_createMessage);
  static CreateRoomRequest? _defaultInstance;

  /// session_id que va a usar el host (debe ser > 0).
  @$pb.TagNumber(1)
  $core.int get sessionId => $_getIZ(0);
  @$pb.TagNumber(1)
  set sessionId($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSessionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearSessionId() => $_clearField(1);

  /// Metadatos opcionales (nombre de sala, organización).
  @$pb.TagNumber(2)
  $core.String get displayName => $_getSZ(1);
  @$pb.TagNumber(2)
  set displayName($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasDisplayName() => $_has(1);
  @$pb.TagNumber(2)
  void clearDisplayName() => $_clearField(2);
}

class CreateRoomResponse extends $pb.GeneratedMessage {
  factory CreateRoomResponse({
    Room? room,
  }) {
    final result = CreateRoomResponse._();
    if (room != null) result.room = room;
    return result;
  }

  CreateRoomResponse._();

  factory CreateRoomResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CreateRoomResponse()..mergeFromBuffer(data, registry);
  factory CreateRoomResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CreateRoomResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CreateRoomResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: CreateRoomResponse.$_createMessage)
    ..aOM<Room>(1, _omitFieldNames ? '' : 'room',
        subBuilder: Room.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CreateRoomResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CreateRoomResponse copyWith(void Function(CreateRoomResponse) updates) =>
      super.copyWith((message) => updates(message as CreateRoomResponse))
          as CreateRoomResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use CreateRoomResponse() / CreateRoomResponse.new instead')
  static CreateRoomResponse create() => CreateRoomResponse._();
  static $pb.GeneratedMessage $_createMessage() => CreateRoomResponse._();
  @$core.override
  CreateRoomResponse createEmptyInstance() => CreateRoomResponse._();
  @$core.pragma('dart2js:noInline')
  static CreateRoomResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CreateRoomResponse>(
          CreateRoomResponse.$_createMessage);
  static CreateRoomResponse? _defaultInstance;

  @$pb.TagNumber(1)
  Room get room => $_getN(0);
  @$pb.TagNumber(1)
  set room(Room value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasRoom() => $_has(0);
  @$pb.TagNumber(1)
  void clearRoom() => $_clearField(1);
  @$pb.TagNumber(1)
  Room ensureRoom() => $_ensure(0);
}

/// Resolución de un código legible a sala.
class GetRoomRequest extends $pb.GeneratedMessage {
  factory GetRoomRequest({
    $core.String? code,
  }) {
    final result = GetRoomRequest._();
    if (code != null) result.code = code;
    return result;
  }

  GetRoomRequest._();

  factory GetRoomRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetRoomRequest()..mergeFromBuffer(data, registry);
  factory GetRoomRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetRoomRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'GetRoomRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: GetRoomRequest.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'code')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetRoomRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetRoomRequest copyWith(void Function(GetRoomRequest) updates) =>
      super.copyWith((message) => updates(message as GetRoomRequest))
          as GetRoomRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use GetRoomRequest() / GetRoomRequest.new instead')
  static GetRoomRequest create() => GetRoomRequest._();
  static $pb.GeneratedMessage $_createMessage() => GetRoomRequest._();
  @$core.override
  GetRoomRequest createEmptyInstance() => GetRoomRequest._();
  @$core.pragma('dart2js:noInline')
  static GetRoomRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<GetRoomRequest>(
          GetRoomRequest.$_createMessage);
  static GetRoomRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get code => $_getSZ(0);
  @$pb.TagNumber(1)
  set code($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasCode() => $_has(0);
  @$pb.TagNumber(1)
  void clearCode() => $_clearField(1);
}

class GetRoomResponse extends $pb.GeneratedMessage {
  factory GetRoomResponse({
    Room? room,
  }) {
    final result = GetRoomResponse._();
    if (room != null) result.room = room;
    return result;
  }

  GetRoomResponse._();

  factory GetRoomResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetRoomResponse()..mergeFromBuffer(data, registry);
  factory GetRoomResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetRoomResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'GetRoomResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: GetRoomResponse.$_createMessage)
    ..aOM<Room>(1, _omitFieldNames ? '' : 'room',
        subBuilder: Room.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetRoomResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetRoomResponse copyWith(void Function(GetRoomResponse) updates) =>
      super.copyWith((message) => updates(message as GetRoomResponse))
          as GetRoomResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use GetRoomResponse() / GetRoomResponse.new instead')
  static GetRoomResponse create() => GetRoomResponse._();
  static $pb.GeneratedMessage $_createMessage() => GetRoomResponse._();
  @$core.override
  GetRoomResponse createEmptyInstance() => GetRoomResponse._();
  @$core.pragma('dart2js:noInline')
  static GetRoomResponse getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<GetRoomResponse>(
          GetRoomResponse.$_createMessage);
  static GetRoomResponse? _defaultInstance;

  @$pb.TagNumber(1)
  Room get room => $_getN(0);
  @$pb.TagNumber(1)
  set room(Room value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasRoom() => $_has(0);
  @$pb.TagNumber(1)
  void clearRoom() => $_clearField(1);
  @$pb.TagNumber(1)
  Room ensureRoom() => $_ensure(0);
}

/// Listado de salas activas.
class ListRoomsRequest extends $pb.GeneratedMessage {
  factory ListRoomsRequest() => ListRoomsRequest._();

  ListRoomsRequest._();

  factory ListRoomsRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListRoomsRequest()..mergeFromBuffer(data, registry);
  factory ListRoomsRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListRoomsRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ListRoomsRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: ListRoomsRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListRoomsRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListRoomsRequest copyWith(void Function(ListRoomsRequest) updates) =>
      super.copyWith((message) => updates(message as ListRoomsRequest))
          as ListRoomsRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ListRoomsRequest() / ListRoomsRequest.new instead')
  static ListRoomsRequest create() => ListRoomsRequest._();
  static $pb.GeneratedMessage $_createMessage() => ListRoomsRequest._();
  @$core.override
  ListRoomsRequest createEmptyInstance() => ListRoomsRequest._();
  @$core.pragma('dart2js:noInline')
  static ListRoomsRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<ListRoomsRequest>(
          ListRoomsRequest.$_createMessage);
  static ListRoomsRequest? _defaultInstance;
}

class ListRoomsResponse extends $pb.GeneratedMessage {
  factory ListRoomsResponse({
    $core.Iterable<Room>? rooms,
  }) {
    final result = ListRoomsResponse._();
    if (rooms != null) result.rooms.addAll(rooms);
    return result;
  }

  ListRoomsResponse._();

  factory ListRoomsResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListRoomsResponse()..mergeFromBuffer(data, registry);
  factory ListRoomsResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListRoomsResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ListRoomsResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: ListRoomsResponse.$_createMessage)
    ..pPM<Room>(1, _omitFieldNames ? '' : 'rooms',
        subBuilder: Room.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListRoomsResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListRoomsResponse copyWith(void Function(ListRoomsResponse) updates) =>
      super.copyWith((message) => updates(message as ListRoomsResponse))
          as ListRoomsResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ListRoomsResponse() / ListRoomsResponse.new instead')
  static ListRoomsResponse create() => ListRoomsResponse._();
  static $pb.GeneratedMessage $_createMessage() => ListRoomsResponse._();
  @$core.override
  ListRoomsResponse createEmptyInstance() => ListRoomsResponse._();
  @$core.pragma('dart2js:noInline')
  static ListRoomsResponse getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<ListRoomsResponse>(
          ListRoomsResponse.$_createMessage);
  static ListRoomsResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<Room> get rooms => $_getList(0);
}

class DeleteRoomRequest extends $pb.GeneratedMessage {
  factory DeleteRoomRequest({
    $core.String? code,
  }) {
    final result = DeleteRoomRequest._();
    if (code != null) result.code = code;
    return result;
  }

  DeleteRoomRequest._();

  factory DeleteRoomRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DeleteRoomRequest()..mergeFromBuffer(data, registry);
  factory DeleteRoomRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DeleteRoomRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'DeleteRoomRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: DeleteRoomRequest.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'code')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DeleteRoomRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DeleteRoomRequest copyWith(void Function(DeleteRoomRequest) updates) =>
      super.copyWith((message) => updates(message as DeleteRoomRequest))
          as DeleteRoomRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use DeleteRoomRequest() / DeleteRoomRequest.new instead')
  static DeleteRoomRequest create() => DeleteRoomRequest._();
  static $pb.GeneratedMessage $_createMessage() => DeleteRoomRequest._();
  @$core.override
  DeleteRoomRequest createEmptyInstance() => DeleteRoomRequest._();
  @$core.pragma('dart2js:noInline')
  static DeleteRoomRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<DeleteRoomRequest>(
          DeleteRoomRequest.$_createMessage);
  static DeleteRoomRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get code => $_getSZ(0);
  @$pb.TagNumber(1)
  set code($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasCode() => $_has(0);
  @$pb.TagNumber(1)
  void clearCode() => $_clearField(1);
}

class DeleteRoomResponse extends $pb.GeneratedMessage {
  factory DeleteRoomResponse({
    $core.bool? deleted,
  }) {
    final result = DeleteRoomResponse._();
    if (deleted != null) result.deleted = deleted;
    return result;
  }

  DeleteRoomResponse._();

  factory DeleteRoomResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DeleteRoomResponse()..mergeFromBuffer(data, registry);
  factory DeleteRoomResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DeleteRoomResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'DeleteRoomResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: DeleteRoomResponse.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'deleted')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DeleteRoomResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DeleteRoomResponse copyWith(void Function(DeleteRoomResponse) updates) =>
      super.copyWith((message) => updates(message as DeleteRoomResponse))
          as DeleteRoomResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use DeleteRoomResponse() / DeleteRoomResponse.new instead')
  static DeleteRoomResponse create() => DeleteRoomResponse._();
  static $pb.GeneratedMessage $_createMessage() => DeleteRoomResponse._();
  @$core.override
  DeleteRoomResponse createEmptyInstance() => DeleteRoomResponse._();
  @$core.pragma('dart2js:noInline')
  static DeleteRoomResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<DeleteRoomResponse>(
          DeleteRoomResponse.$_createMessage);
  static DeleteRoomResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get deleted => $_getBF(0);
  @$pb.TagNumber(1)
  set deleted($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasDeleted() => $_has(0);
  @$pb.TagNumber(1)
  void clearDeleted() => $_clearField(1);
}

/// Límites y capacidad del servidor.
class ServerInfoRequest extends $pb.GeneratedMessage {
  factory ServerInfoRequest() => ServerInfoRequest._();

  ServerInfoRequest._();

  factory ServerInfoRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerInfoRequest()..mergeFromBuffer(data, registry);
  factory ServerInfoRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerInfoRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ServerInfoRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: ServerInfoRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerInfoRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerInfoRequest copyWith(void Function(ServerInfoRequest) updates) =>
      super.copyWith((message) => updates(message as ServerInfoRequest))
          as ServerInfoRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ServerInfoRequest() / ServerInfoRequest.new instead')
  static ServerInfoRequest create() => ServerInfoRequest._();
  static $pb.GeneratedMessage $_createMessage() => ServerInfoRequest._();
  @$core.override
  ServerInfoRequest createEmptyInstance() => ServerInfoRequest._();
  @$core.pragma('dart2js:noInline')
  static ServerInfoRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<ServerInfoRequest>(
          ServerInfoRequest.$_createMessage);
  static ServerInfoRequest? _defaultInstance;
}

class ServerInfoResponse extends $pb.GeneratedMessage {
  factory ServerInfoResponse({
    $core.String? version,
    $core.int? protocolVersion,
    $core.int? maxSessions,
    $core.int? maxPeersPerSession,
    $fixnum.Int64? activeSessions,
    $fixnum.Int64? activePeers,
    $core.String? uptimeSecs,
  }) {
    final result = ServerInfoResponse._();
    if (version != null) result.version = version;
    if (protocolVersion != null) result.protocolVersion = protocolVersion;
    if (maxSessions != null) result.maxSessions = maxSessions;
    if (maxPeersPerSession != null)
      result.maxPeersPerSession = maxPeersPerSession;
    if (activeSessions != null) result.activeSessions = activeSessions;
    if (activePeers != null) result.activePeers = activePeers;
    if (uptimeSecs != null) result.uptimeSecs = uptimeSecs;
    return result;
  }

  ServerInfoResponse._();

  factory ServerInfoResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerInfoResponse()..mergeFromBuffer(data, registry);
  factory ServerInfoResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerInfoResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ServerInfoResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: ServerInfoResponse.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'version')
    ..aI(2, _omitFieldNames ? '' : 'protocolVersion',
        fieldType: $pb.PbFieldType.OU3)
    ..aI(3, _omitFieldNames ? '' : 'maxSessions')
    ..aI(4, _omitFieldNames ? '' : 'maxPeersPerSession')
    ..aInt64(5, _omitFieldNames ? '' : 'activeSessions')
    ..aInt64(6, _omitFieldNames ? '' : 'activePeers')
    ..aOS(7, _omitFieldNames ? '' : 'uptimeSecs')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerInfoResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerInfoResponse copyWith(void Function(ServerInfoResponse) updates) =>
      super.copyWith((message) => updates(message as ServerInfoResponse))
          as ServerInfoResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ServerInfoResponse() / ServerInfoResponse.new instead')
  static ServerInfoResponse create() => ServerInfoResponse._();
  static $pb.GeneratedMessage $_createMessage() => ServerInfoResponse._();
  @$core.override
  ServerInfoResponse createEmptyInstance() => ServerInfoResponse._();
  @$core.pragma('dart2js:noInline')
  static ServerInfoResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ServerInfoResponse>(
          ServerInfoResponse.$_createMessage);
  static ServerInfoResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get version => $_getSZ(0);
  @$pb.TagNumber(1)
  set version($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasVersion() => $_has(0);
  @$pb.TagNumber(1)
  void clearVersion() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.int get protocolVersion => $_getIZ(1);
  @$pb.TagNumber(2)
  set protocolVersion($core.int value) => $_setUnsignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasProtocolVersion() => $_has(1);
  @$pb.TagNumber(2)
  void clearProtocolVersion() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.int get maxSessions => $_getIZ(2);
  @$pb.TagNumber(3)
  set maxSessions($core.int value) => $_setSignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasMaxSessions() => $_has(2);
  @$pb.TagNumber(3)
  void clearMaxSessions() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.int get maxPeersPerSession => $_getIZ(3);
  @$pb.TagNumber(4)
  set maxPeersPerSession($core.int value) => $_setSignedInt32(3, value);
  @$pb.TagNumber(4)
  $core.bool hasMaxPeersPerSession() => $_has(3);
  @$pb.TagNumber(4)
  void clearMaxPeersPerSession() => $_clearField(4);

  @$pb.TagNumber(5)
  $fixnum.Int64 get activeSessions => $_getI64(4);
  @$pb.TagNumber(5)
  set activeSessions($fixnum.Int64 value) => $_setInt64(4, value);
  @$pb.TagNumber(5)
  $core.bool hasActiveSessions() => $_has(4);
  @$pb.TagNumber(5)
  void clearActiveSessions() => $_clearField(5);

  @$pb.TagNumber(6)
  $fixnum.Int64 get activePeers => $_getI64(5);
  @$pb.TagNumber(6)
  set activePeers($fixnum.Int64 value) => $_setInt64(5, value);
  @$pb.TagNumber(6)
  $core.bool hasActivePeers() => $_has(5);
  @$pb.TagNumber(6)
  void clearActivePeers() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.String get uptimeSecs => $_getSZ(6);
  @$pb.TagNumber(7)
  set uptimeSecs($core.String value) => $_setString(6, value);
  @$pb.TagNumber(7)
  $core.bool hasUptimeSecs() => $_has(6);
  @$pb.TagNumber(7)
  void clearUptimeSecs() => $_clearField(7);
}

/// Snapshot de salud y métricas agregadas.
class ServerHealthRequest extends $pb.GeneratedMessage {
  factory ServerHealthRequest() => ServerHealthRequest._();

  ServerHealthRequest._();

  factory ServerHealthRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerHealthRequest()..mergeFromBuffer(data, registry);
  factory ServerHealthRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerHealthRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ServerHealthRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: ServerHealthRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerHealthRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerHealthRequest copyWith(void Function(ServerHealthRequest) updates) =>
      super.copyWith((message) => updates(message as ServerHealthRequest))
          as ServerHealthRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core
      .Deprecated('Use ServerHealthRequest() / ServerHealthRequest.new instead')
  static ServerHealthRequest create() => ServerHealthRequest._();
  static $pb.GeneratedMessage $_createMessage() => ServerHealthRequest._();
  @$core.override
  ServerHealthRequest createEmptyInstance() => ServerHealthRequest._();
  @$core.pragma('dart2js:noInline')
  static ServerHealthRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ServerHealthRequest>(
          ServerHealthRequest.$_createMessage);
  static ServerHealthRequest? _defaultInstance;
}

class ServerHealthResponse extends $pb.GeneratedMessage {
  factory ServerHealthResponse({
    $core.String? status,
    $fixnum.Int64? packetsIn,
    $fixnum.Int64? packetsOut,
    $fixnum.Int64? bytesIn,
    $fixnum.Int64? bytesOut,
    $fixnum.Int64? droppedInvalid,
    $fixnum.Int64? activeSessions,
  }) {
    final result = ServerHealthResponse._();
    if (status != null) result.status = status;
    if (packetsIn != null) result.packetsIn = packetsIn;
    if (packetsOut != null) result.packetsOut = packetsOut;
    if (bytesIn != null) result.bytesIn = bytesIn;
    if (bytesOut != null) result.bytesOut = bytesOut;
    if (droppedInvalid != null) result.droppedInvalid = droppedInvalid;
    if (activeSessions != null) result.activeSessions = activeSessions;
    return result;
  }

  ServerHealthResponse._();

  factory ServerHealthResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerHealthResponse()..mergeFromBuffer(data, registry);
  factory ServerHealthResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ServerHealthResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ServerHealthResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: ServerHealthResponse.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'status')
    ..a<$fixnum.Int64>(
        2, _omitFieldNames ? '' : 'packetsIn', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(
        3, _omitFieldNames ? '' : 'packetsOut', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(4, _omitFieldNames ? '' : 'bytesIn', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(
        5, _omitFieldNames ? '' : 'bytesOut', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(
        6, _omitFieldNames ? '' : 'droppedInvalid', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..aInt64(7, _omitFieldNames ? '' : 'activeSessions')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerHealthResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ServerHealthResponse copyWith(void Function(ServerHealthResponse) updates) =>
      super.copyWith((message) => updates(message as ServerHealthResponse))
          as ServerHealthResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use ServerHealthResponse() / ServerHealthResponse.new instead')
  static ServerHealthResponse create() => ServerHealthResponse._();
  static $pb.GeneratedMessage $_createMessage() => ServerHealthResponse._();
  @$core.override
  ServerHealthResponse createEmptyInstance() => ServerHealthResponse._();
  @$core.pragma('dart2js:noInline')
  static ServerHealthResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ServerHealthResponse>(
          ServerHealthResponse.$_createMessage);
  static ServerHealthResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get status => $_getSZ(0);
  @$pb.TagNumber(1)
  set status($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasStatus() => $_has(0);
  @$pb.TagNumber(1)
  void clearStatus() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get packetsIn => $_getI64(1);
  @$pb.TagNumber(2)
  set packetsIn($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasPacketsIn() => $_has(1);
  @$pb.TagNumber(2)
  void clearPacketsIn() => $_clearField(2);

  @$pb.TagNumber(3)
  $fixnum.Int64 get packetsOut => $_getI64(2);
  @$pb.TagNumber(3)
  set packetsOut($fixnum.Int64 value) => $_setInt64(2, value);
  @$pb.TagNumber(3)
  $core.bool hasPacketsOut() => $_has(2);
  @$pb.TagNumber(3)
  void clearPacketsOut() => $_clearField(3);

  @$pb.TagNumber(4)
  $fixnum.Int64 get bytesIn => $_getI64(3);
  @$pb.TagNumber(4)
  set bytesIn($fixnum.Int64 value) => $_setInt64(3, value);
  @$pb.TagNumber(4)
  $core.bool hasBytesIn() => $_has(3);
  @$pb.TagNumber(4)
  void clearBytesIn() => $_clearField(4);

  @$pb.TagNumber(5)
  $fixnum.Int64 get bytesOut => $_getI64(4);
  @$pb.TagNumber(5)
  set bytesOut($fixnum.Int64 value) => $_setInt64(4, value);
  @$pb.TagNumber(5)
  $core.bool hasBytesOut() => $_has(4);
  @$pb.TagNumber(5)
  void clearBytesOut() => $_clearField(5);

  @$pb.TagNumber(6)
  $fixnum.Int64 get droppedInvalid => $_getI64(5);
  @$pb.TagNumber(6)
  set droppedInvalid($fixnum.Int64 value) => $_setInt64(5, value);
  @$pb.TagNumber(6)
  $core.bool hasDroppedInvalid() => $_has(5);
  @$pb.TagNumber(6)
  void clearDroppedInvalid() => $_clearField(6);

  @$pb.TagNumber(7)
  $fixnum.Int64 get activeSessions => $_getI64(6);
  @$pb.TagNumber(7)
  set activeSessions($fixnum.Int64 value) => $_setInt64(6, value);
  @$pb.TagNumber(7)
  $core.bool hasActiveSessions() => $_has(6);
  @$pb.TagNumber(7)
  void clearActiveSessions() => $_clearField(7);
}

/// Evento de sala para WatchRoom (streaming servidor).
class RoomEvent extends $pb.GeneratedMessage {
  factory RoomEvent({
    RoomEvent_EventType? type,
    $core.String? roomCode,
    $core.int? sessionId,
    $fixnum.Int64? atUnixMs,
    $core.int? peerSsrc,
  }) {
    final result = RoomEvent._();
    if (type != null) result.type = type;
    if (roomCode != null) result.roomCode = roomCode;
    if (sessionId != null) result.sessionId = sessionId;
    if (atUnixMs != null) result.atUnixMs = atUnixMs;
    if (peerSsrc != null) result.peerSsrc = peerSsrc;
    return result;
  }

  RoomEvent._();

  factory RoomEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      RoomEvent()..mergeFromBuffer(data, registry);
  factory RoomEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      RoomEvent()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'RoomEvent',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: RoomEvent.$_createMessage)
    ..aE<RoomEvent_EventType>(1, _omitFieldNames ? '' : 'type',
        enumValues: RoomEvent_EventType.values)
    ..aOS(2, _omitFieldNames ? '' : 'roomCode')
    ..aI(3, _omitFieldNames ? '' : 'sessionId', fieldType: $pb.PbFieldType.OU3)
    ..aInt64(4, _omitFieldNames ? '' : 'atUnixMs')
    ..aI(5, _omitFieldNames ? '' : 'peerSsrc', fieldType: $pb.PbFieldType.OU3)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RoomEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RoomEvent copyWith(void Function(RoomEvent) updates) =>
      super.copyWith((message) => updates(message as RoomEvent)) as RoomEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use RoomEvent() / RoomEvent.new instead')
  static RoomEvent create() => RoomEvent._();
  static $pb.GeneratedMessage $_createMessage() => RoomEvent._();
  @$core.override
  RoomEvent createEmptyInstance() => RoomEvent._();
  @$core.pragma('dart2js:noInline')
  static RoomEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<RoomEvent>(RoomEvent.$_createMessage);
  static RoomEvent? _defaultInstance;

  @$pb.TagNumber(1)
  RoomEvent_EventType get type => $_getN(0);
  @$pb.TagNumber(1)
  set type(RoomEvent_EventType value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasType() => $_has(0);
  @$pb.TagNumber(1)
  void clearType() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get roomCode => $_getSZ(1);
  @$pb.TagNumber(2)
  set roomCode($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasRoomCode() => $_has(1);
  @$pb.TagNumber(2)
  void clearRoomCode() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.int get sessionId => $_getIZ(2);
  @$pb.TagNumber(3)
  set sessionId($core.int value) => $_setUnsignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasSessionId() => $_has(2);
  @$pb.TagNumber(3)
  void clearSessionId() => $_clearField(3);

  /// Momento del evento (unix ms; 0 = desconocido).
  @$pb.TagNumber(4)
  $fixnum.Int64 get atUnixMs => $_getI64(3);
  @$pb.TagNumber(4)
  set atUnixMs($fixnum.Int64 value) => $_setInt64(3, value);
  @$pb.TagNumber(4)
  $core.bool hasAtUnixMs() => $_has(3);
  @$pb.TagNumber(4)
  void clearAtUnixMs() => $_clearField(4);

  /// Identificador del peer (SSRC o dirección) cuando aplica.
  @$pb.TagNumber(5)
  $core.int get peerSsrc => $_getIZ(4);
  @$pb.TagNumber(5)
  set peerSsrc($core.int value) => $_setUnsignedInt32(4, value);
  @$pb.TagNumber(5)
  $core.bool hasPeerSsrc() => $_has(4);
  @$pb.TagNumber(5)
  void clearPeerSsrc() => $_clearField(5);
}

class WatchRoomRequest extends $pb.GeneratedMessage {
  factory WatchRoomRequest({
    $core.String? roomCode,
  }) {
    final result = WatchRoomRequest._();
    if (roomCode != null) result.roomCode = roomCode;
    return result;
  }

  WatchRoomRequest._();

  factory WatchRoomRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      WatchRoomRequest()..mergeFromBuffer(data, registry);
  factory WatchRoomRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      WatchRoomRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'WatchRoomRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: WatchRoomRequest.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'roomCode')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  WatchRoomRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  WatchRoomRequest copyWith(void Function(WatchRoomRequest) updates) =>
      super.copyWith((message) => updates(message as WatchRoomRequest))
          as WatchRoomRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use WatchRoomRequest() / WatchRoomRequest.new instead')
  static WatchRoomRequest create() => WatchRoomRequest._();
  static $pb.GeneratedMessage $_createMessage() => WatchRoomRequest._();
  @$core.override
  WatchRoomRequest createEmptyInstance() => WatchRoomRequest._();
  @$core.pragma('dart2js:noInline')
  static WatchRoomRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<WatchRoomRequest>(
          WatchRoomRequest.$_createMessage);
  static WatchRoomRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get roomCode => $_getSZ(0);
  @$pb.TagNumber(1)
  set roomCode($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasRoomCode() => $_has(0);
  @$pb.TagNumber(1)
  void clearRoomCode() => $_clearField(1);
}

const $core.bool _omitFieldNames =
    $core.bool.fromEnvironment('protobuf.omit_field_names');
const $core.bool _omitMessageNames =
    $core.bool.fromEnvironment('protobuf.omit_message_names');
