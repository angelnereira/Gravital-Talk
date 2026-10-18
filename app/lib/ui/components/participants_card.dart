/// Lista de participantes de la sala (GEUI).
library;

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../services/room_events.dart';
import '../../services/session_controller.dart';
import '../semantic_colors.dart';
import '../tokens.dart';
import 'section_card.dart';

// `Participant` no se redefine aquí: lo aporta `services/room_events.dart`, que
// es donde vive la etiqueta estable («Host», «Peer 0x…») y la descripción de
// estado. Duplicarla hacía que esta tarjeta enseñara otro texto que el resto de
// la app y las pruebas no reconocían.

/// Lista de participantes de la sala.
class ParticipantsCard extends StatelessWidget {
  const ParticipantsCard({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final sem = SemanticColors.of(context);
    final events = context.watch<RoomEvents>();
    final controller = context.watch<SessionController>();

    final participants = <Participant>[
      Participant(
        ssrc: controller.sessionId,
        isLocal: true,
        hasFloor: events.floorHolder == controller.sessionId,
      ),
      for (final ssrc in events.peers)
        Participant(
          ssrc: ssrc,
          hasFloor: events.floorHolder == ssrc,
        ),
    ];

    return SectionCard(
      title: 'Participantes',
      subtitle: switch (events.isConnected) {
        true => '${participants.length} en la sala',
        false when events.roomCode == null => 'Sin observación de sala',
        false => 'Observación de sala perdida',
      },
      trailing: events.isConnected
          ? Icon(Icons.circle, size: 10, color: sem.success.color)
          : Icon(Icons.cloud_off, size: 18, color: scheme.outline),
      child: Column(
        children: [
          for (final p in participants)
            Padding(
              padding: const EdgeInsets.only(bottom: NSpace.sm),
              child: Row(
                children: [
                  Container(
                    width: 32,
                    height: 32,
                    decoration: BoxDecoration(
                      color: p.hasFloor
                          ? sem.pttActive.color.withValues(alpha: 0.15)
                          : scheme.surfaceContainerHighest,
                      shape: BoxShape.circle,
                    ),
                    child: Icon(
                      p.hasFloor
                          ? Icons.graphic_eq
                          : (p.isLocal ? Icons.person : Icons.people),
                      size: 16,
                      color: p.hasFloor
                          ? sem.pttActive.color
                          : scheme.onSurfaceVariant,
                    ),
                  ),
                  const SizedBox(width: NSpace.md),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          p.isLocal ? '${p.label} (tú)' : p.label,
                          style: theme.textTheme.bodyMedium?.copyWith(
                            fontWeight:
                                p.hasFloor ? FontWeight.w700 : FontWeight.w500,
                          ),
                        ),
                        Text(
                          p.statusLabel,
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: p.hasFloor
                                ? sem.pttActive.color
                                : scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  if (p.hasFloor)
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: NSpace.sm,
                        vertical: NSpace.xs,
                      ),
                      decoration: BoxDecoration(
                        color: sem.pttActive.color,
                        borderRadius: BorderRadius.circular(NShape.full),
                      ),
                      child: Text(
                        'TURNO',
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: sem.pttActive.onColor,
                          fontWeight: FontWeight.w800,
                          letterSpacing: 0.6,
                        ),
                      ),
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}
