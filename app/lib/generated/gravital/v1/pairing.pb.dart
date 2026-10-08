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

import 'package:fixnum/fixnum.dart' as $fixnum;
import 'package:protobuf/protobuf.dart' as $pb;

import 'pairing.pbenum.dart';

export 'package:protobuf/protobuf.dart' show GeneratedMessageGenericExtensions;

export 'pairing.pbenum.dart';

/// Parámetros de conexión que comparte un host P2P.
class PairingOffer extends $pb.GeneratedMessage {
  factory PairingOffer({
    $core.String? uri,
    $core.String? deviceName,
    $core.int? sessionId,
    $fixnum.Int64? generatedAtUnixMs,
  }) {
    final result = PairingOffer._();
    if (uri != null) result.uri = uri;
    if (deviceName != null) result.deviceName = deviceName;
    if (sessionId != null) result.sessionId = sessionId;
    if (generatedAtUnixMs != null) result.generatedAtUnixMs = generatedAtUnixMs;
    return result;
  }

  PairingOffer._();

  factory PairingOffer.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PairingOffer()..mergeFromBuffer(data, registry);
  factory PairingOffer.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PairingOffer()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PairingOffer',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: PairingOffer.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'uri')
    ..aOS(2, _omitFieldNames ? '' : 'deviceName')
    ..aI(3, _omitFieldNames ? '' : 'sessionId', fieldType: $pb.PbFieldType.OU3)
    ..aInt64(4, _omitFieldNames ? '' : 'generatedAtUnixMs')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PairingOffer clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PairingOffer copyWith(void Function(PairingOffer) updates) =>
      super.copyWith((message) => updates(message as PairingOffer))
          as PairingOffer;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PairingOffer() / PairingOffer.new instead')
  static PairingOffer create() => PairingOffer._();
  static $pb.GeneratedMessage $_createMessage() => PairingOffer._();
  @$core.override
  PairingOffer createEmptyInstance() => PairingOffer._();
  @$core.pragma('dart2js:noInline')
  static PairingOffer getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PairingOffer>(
          PairingOffer.$_createMessage);
  static PairingOffer? _defaultInstance;

  /// URI canónica: gravital-talk://pair?v=1&lan=<ip>:<port>&pub=<ip>:<port>
  @$pb.TagNumber(1)
  $core.String get uri => $_getSZ(0);
  @$pb.TagNumber(1)
  set uri($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasUri() => $_has(0);
  @$pb.TagNumber(1)
  void clearUri() => $_clearField(1);

  /// Descripción humana (nombre del dispositivo).
  @$pb.TagNumber(2)
  $core.String get deviceName => $_getSZ(1);
  @$pb.TagNumber(2)
  set deviceName($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasDeviceName() => $_has(1);
  @$pb.TagNumber(2)
  void clearDeviceName() => $_clearField(2);

  /// session_id que usará el peer al conectar (0 = lo decide el servidor).
  @$pb.TagNumber(3)
  $core.int get sessionId => $_getIZ(2);
  @$pb.TagNumber(3)
  set sessionId($core.int value) => $_setUnsignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasSessionId() => $_has(2);
  @$pb.TagNumber(3)
  void clearSessionId() => $_clearField(3);

  /// Marca de tiempo de generación.
  @$pb.TagNumber(4)
  $fixnum.Int64 get generatedAtUnixMs => $_getI64(3);
  @$pb.TagNumber(4)
  set generatedAtUnixMs($fixnum.Int64 value) => $_setInt64(3, value);
  @$pb.TagNumber(4)
  $core.bool hasGeneratedAtUnixMs() => $_has(3);
  @$pb.TagNumber(4)
  void clearGeneratedAtUnixMs() => $_clearField(4);
}

/// Respuesta del peer que acepta el emparejamiento.
class PairingAccept extends $pb.GeneratedMessage {
  factory PairingAccept({
    $core.bool? accepted,
    $core.String? rejectReason,
    $core.String? fallbackRelay,
  }) {
    final result = PairingAccept._();
    if (accepted != null) result.accepted = accepted;
    if (rejectReason != null) result.rejectReason = rejectReason;
    if (fallbackRelay != null) result.fallbackRelay = fallbackRelay;
    return result;
  }

  PairingAccept._();

  factory PairingAccept.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PairingAccept()..mergeFromBuffer(data, registry);
  factory PairingAccept.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PairingAccept()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PairingAccept',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: PairingAccept.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'accepted')
    ..aOS(2, _omitFieldNames ? '' : 'rejectReason')
    ..aOS(3, _omitFieldNames ? '' : 'fallbackRelay')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PairingAccept clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PairingAccept copyWith(void Function(PairingAccept) updates) =>
      super.copyWith((message) => updates(message as PairingAccept))
          as PairingAccept;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PairingAccept() / PairingAccept.new instead')
  static PairingAccept create() => PairingAccept._();
  static $pb.GeneratedMessage $_createMessage() => PairingAccept._();
  @$core.override
  PairingAccept createEmptyInstance() => PairingAccept._();
  @$core.pragma('dart2js:noInline')
  static PairingAccept getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PairingAccept>(
          PairingAccept.$_createMessage);
  static PairingAccept? _defaultInstance;

  /// Rol que adoptará el peer: comenzar el handshake como cliente.
  @$pb.TagNumber(1)
  $core.bool get accepted => $_getBF(0);
  @$pb.TagNumber(1)
  set accepted($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasAccepted() => $_has(0);
  @$pb.TagNumber(1)
  void clearAccepted() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get rejectReason => $_getSZ(1);
  @$pb.TagNumber(2)
  set rejectReason($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasRejectReason() => $_has(1);
  @$pb.TagNumber(2)
  void clearRejectReason() => $_clearField(2);

  /// Dirección de vuelta al host (para casos de redes con relay intermedio).
  @$pb.TagNumber(3)
  $core.String get fallbackRelay => $_getSZ(2);
  @$pb.TagNumber(3)
  set fallbackRelay($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasFallbackRelay() => $_has(2);
  @$pb.TagNumber(3)
  void clearFallbackRelay() => $_clearField(3);
}

/// Resultado del intento de conexión.
class PairingResult extends $pb.GeneratedMessage {
  factory PairingResult({
    PairingResult_Status? status,
    $core.int? negotiatedSessionId,
    $core.String? errorDetail,
  }) {
    final result = PairingResult._();
    if (status != null) result.status = status;
    if (negotiatedSessionId != null)
      result.negotiatedSessionId = negotiatedSessionId;
    if (errorDetail != null) result.errorDetail = errorDetail;
    return result;
  }

  PairingResult._();

  factory PairingResult.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PairingResult()..mergeFromBuffer(data, registry);
  factory PairingResult.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PairingResult()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PairingResult',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'gravital.v1'),
      createEmptyInstance: PairingResult.$_createMessage)
    ..aE<PairingResult_Status>(1, _omitFieldNames ? '' : 'status',
        enumValues: PairingResult_Status.values)
    ..aI(2, _omitFieldNames ? '' : 'negotiatedSessionId',
        fieldType: $pb.PbFieldType.OU3)
    ..aOS(3, _omitFieldNames ? '' : 'errorDetail')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PairingResult clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PairingResult copyWith(void Function(PairingResult) updates) =>
      super.copyWith((message) => updates(message as PairingResult))
          as PairingResult;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PairingResult() / PairingResult.new instead')
  static PairingResult create() => PairingResult._();
  static $pb.GeneratedMessage $_createMessage() => PairingResult._();
  @$core.override
  PairingResult createEmptyInstance() => PairingResult._();
  @$core.pragma('dart2js:noInline')
  static PairingResult getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PairingResult>(
          PairingResult.$_createMessage);
  static PairingResult? _defaultInstance;

  @$pb.TagNumber(1)
  PairingResult_Status get status => $_getN(0);
  @$pb.TagNumber(1)
  set status(PairingResult_Status value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasStatus() => $_has(0);
  @$pb.TagNumber(1)
  void clearStatus() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.int get negotiatedSessionId => $_getIZ(1);
  @$pb.TagNumber(2)
  set negotiatedSessionId($core.int value) => $_setUnsignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasNegotiatedSessionId() => $_has(1);
  @$pb.TagNumber(2)
  void clearNegotiatedSessionId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get errorDetail => $_getSZ(2);
  @$pb.TagNumber(3)
  set errorDetail($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasErrorDetail() => $_has(2);
  @$pb.TagNumber(3)
  void clearErrorDetail() => $_clearField(3);
}

const $core.bool _omitFieldNames =
    $core.bool.fromEnvironment('protobuf.omit_field_names');
const $core.bool _omitMessageNames =
    $core.bool.fromEnvironment('protobuf.omit_message_names');
