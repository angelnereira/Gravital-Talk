cmake_minimum_required(VERSION 3.10)
set(PROJECT_NAME "flutter_sound_linux")
project(${PROJECT_NAME} LANGUAGES CXX)

set(PLUGIN_NAME "${PROJECT_NAME}_plugin")

# Libreria vacia: el generador de CMake de Flutter exige que el target de cada
# plugin sea una libreria real (usa $<TARGET_FILE:...>). El contenido esta en
# la cabecera, declarada `inline` para no depender de enlazar nada.
add_library(${PLUGIN_NAME} SHARED
  "flutter_sound_linux.cc"
)
apply_standard_settings(${PLUGIN_NAME})
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
