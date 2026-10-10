#ifndef FLUTTER_SOUND_LINUX_PLUGIN_H_
#define FLUTTER_SOUND_LINUX_PLUGIN_H_

// Plugin nativo vacio para Linux.
//
// La implementacion real (un no-op via method channel) esta en Dart; ver
// lib/flutter_sound_linux.dart.
//
// Se declara `inline` a proposito: el generador de CMake de Flutter usa
// $<TARGET_FILE:...> sobre este plugin y obliga a que sea una libreria real,
// pero enlazarla contra el binario no aporta nada. Definiendola aqui, cualquier
// unidad que incluya esta cabecera ya la tiene.
//
// El parámetro es `void*` y no `FlPluginRegistrar*` para no depender de las
// cabeceras del motor, que no estan en el include path de este plugin.
inline void flutter_sound_linux_plugin_register_with_registrar(
    void* registrar) {
  (void)registrar;
}

#endif  // FLUTTER_SOUND_LINUX_PLUGIN_H_
