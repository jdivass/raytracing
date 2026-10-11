# Raytracer de un diorama voxel

Proyecto en Rust que renderiza un diorama de estilo Minecraft mediante trazado de rayos. Carga una construcción desde un schematic, usa sus modelos y texturas y permite recorrerla en una ventana a pantalla completa.

## ⭐ Características

- **Mundo voxel:** carga `assets/lonlonranchclean.schem`, interpreta su paleta de bloques y consulta las intersecciones con una marcha por celdas.
- **Modelos y texturas:** obtiene texturas de `textures/assets/minecraft` y usa los modelos disponibles para representar bloques con formas distintas a un cubo.
- **Iluminación:** combina el ciclo solar y lunar con luz ambiental y luz de materiales emisivos.
- **Ciclo de día y noche:** el sol amarillo y la luna se desplazan por el cielo; la iluminación y el color del cielo cambian con la hora.
- **Materiales:** admiten albedo, especularidad, transparencia, emisión, reflexión, refracción e índice de refracción.
- **Agua:** usa reflexión y refracción con un índice de refracción de 1.333.
- **Mapas normales:** `assets/normal_bricks.jpg` añade relieve aparente a ladrillos y losas/escaleras de ladrillo; `assets/normal_rocks.jpg` se aplica al cobllestone también.
- **Vista panorámica:** `assets/skybox.jpg` sirve como fondo del cielo.
- **Vista interactiva:** durante el movimiento o el cambio de hora, el programa reduce la resolución del render de forma adaptativa y limita los rebotes secundarios. Al detenerse, genera una imagen a resolución completa.
- **Ejecución paralela:** distribuye el render de las filas de píxeles entre los hilos disponibles mediante Rayon.

## 🛠️ Requisitos

- Rust y Cargo, con una versión que admita la edición 2024.
- Un entorno gráfico compatible con Raylib.

Las dependencias de Rust se declaran en `Cargo.toml`.

## 🤖 Ejecutar

Desde la raíz del proyecto:

```sh
cargo run --release
```

La ventana se abre a pantalla completa. Para compilar sin iniciar la aplicación:

```sh
cargo build --release
```

## 🎮 Controles

| Tecla o entrada | Acción |
| --- | --- |
| Ratón | Mirar alrededor |
| `W` / `S` | Avanzar / retroceder |
| `A` / `D` | Desplazarse a izquierda / derecha |
| `E` / `Q` | Subir / bajar |
| `R` / `F` | Acercar / alejar mediante el campo de visión, sin mover la cámara |
| `Z` / `C` | Retroceder / avanzar el ciclo de día y noche |
| `Esc` | Cerrar la aplicación |

Mantener una tecla de movimiento, zoom o tiempo activa el modo de previsualización pixelada. Al dejar de interactuar, el render completo se actualiza automáticamente.

## 📂 Estructura principal

- `src/main.rs`: ventana, ciclo de iluminación, trazado de rayos y renderizado.
- `src/world.rs`: lectura del schematic, bloques, modelos e intersecciones.
- `src/material.rs`: materiales, texturas y muestreo de mapas normales.
- `src/camera.rs`: cámara libre, controles de movimiento y zoom.
- `src/framebuffer.rs`: almacenamiento y presentación del resultado.
- `assets/`: schematic, cielo y mapas normales usados por la escena.
- `textures/assets/minecraft/`: recursos de texturas y modelos de bloques.

## 📃 Puntos del proyecto implementados

| Característica | Punteo |
| -------------- | ------ |
|Criterio subjetivo. Por qué tan compleja sea su escena: 34, 500 bloques | 20 |
|Criterio subjetivo. Por su implementación de progra paralela y optimización de algoritmos: Uso de prerenderizado y paralelización| 10 |
| Por implementar rotación en su diorama y dejar que la cámara se acerque y aleje | 10 |
| 5 puntos por cada material diferente que implementen, para un máximo de 5 para que el material cuente, debe tener su propia textura, y sus propios parámetros para albedo, specular, transparencia y reflectividad | 25 |
| Por implementar refracción en al menos uno de sus materiales (debe tener sentido contextual en su escena): agua en la fuente del centro | 10 | 
| Por implementar reflexión en al menos uno de sus materiales: agua en la fuente del centro | 5 |
| Por implementar mapas normales en al menos un material: cobblestone y ladrillos | 10 |
| Por implementar al menos un material emisivo: linternas | 10 |
| Por implementar un skybox para su material: fondo con imagen | 10 |

## 🤖 Autor
Julián Divas - 24687
