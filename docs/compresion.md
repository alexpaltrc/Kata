# La compresión PNG de Kata

Kata guarda cada captura como PNG con un compresor propio, escrito desde cero, **sin cambiar un
solo píxel**. Frente al que trae Windows, en nuestro banco de 19 capturas los archivos pesan **18 %
menos de media**. En el uso diario se gana menos: entre 5 y 8 % en ventanas, y con un fondo de
pantalla que es una foto, 0,5 % más. El plan para ir mucho más lejos está al final.

Este documento cuenta por qué lo hicimos, cómo funciona, qué probamos (también lo que no
funcionó) y qué decidimos, con los números de cada experimento.

## Objetivo

- **Archivos más pequeños que los de Windows**, con exactamente la misma imagen.
- **Todo nuestro.** No dependemos de nada ni de nadie: ni librerías ni código ajeno. Partimos de
  las especificaciones públicas y de lo que sabemos hoy, con tecnologías modernas.
- **Que nadie tenga que pensarlo.** No hay ajustes de calidad ni de nivel: Kata elige sola.
- **Que siga mejorando.** Es una de las funciones estrella de Kata, así que cada idea nueva se
  prueba, se mide y se anota aquí.

## Lo que decidimos no hacer, y por qué

| No hicimos | Por qué |
|---|---|
| Usar librerías de compresión (zlib, libpng, miniz u otras) | Queremos entender y controlar cada byte, sin depender del trabajo de nadie. |
| Copiar o adaptar código ajeno | Todo es nuestro. Solo nos basamos en las especificaciones públicas. |
| Cambiar de formato (WebP, AVIF, JPEG XL…) | El archivo sigue siendo un PNG normal, que cualquier programa abre: Word, un navegador, una wiki. |
| Comprimir con pérdida | Las capturas son para manuales: el texto tiene que verse exacto. Ni un píxel cambia. |
| Añadir un ajuste de compresión | Ante la duda, la opción más simple. Kata decide sola. |
| Hacer esperar al usuario | La captura se copia al portapapeles al instante y el archivo se guarda en segundo plano. |

## ¿Es legal? ¿Cuesta algo?

No hay ningún problema ni costo. PNG es un estándar abierto (W3C e ISO/IEC 15948). DEFLATE, la
compresión que usa por dentro, y zlib, su envoltorio, son estándares abiertos (RFC 1951 y RFC
1950), pensados para usarse libremente y sin patentes. Escribimos el código a partir de esos
documentos.

## Qué es nuestro y qué no

El código es 100 % nuestro, igual que las decisiones: qué probar, cómo medirlo y qué quedarse.
Algunas técnicas son conocidas desde hace décadas y forman parte del conocimiento común de la
informática: las cadenas de búsqueda, los códigos de Huffman o el método *package-merge*. Las
escribimos nosotros, desde su definición. Para elegir el filtro de cada fila (paso 2), en vez de
la regla habitual, usamos una medida propia: contar pares de bytes distintos.

## Cómo funciona, sin tecnicismos

Un PNG guarda la imagen en tres pasos. Kata los hace así:

1. **Contar los colores.** Muchas ventanas tienen pocos colores. Si hay 256 o menos, Kata hace una
   paleta, como un cuadro de pintar por números: cada píxel guarda solo el número de su color.
   Con 2 colores basta 1 bit por píxel; con 16, 4 bits. Si hay más colores, cada píxel guarda su
   rojo, verde y azul.
2. **Volver repetitiva cada fila.** Una fila puede guardarse tal cual o como la diferencia con el
   píxel de la izquierda, el de arriba o una mezcla de ambos. Son cinco formas, llamadas filtros.
   Kata prueba las cinco en cada fila y se queda con la que deja la fila más repetitiva.
3. **Comprimir.** Primero busca repeticiones y las cambia por avisos cortos del tipo «repite los
   40 bytes que hubo hace 300». Luego le da códigos cortos a lo que más aparece y largos a lo raro,
   como el código Morse le da un solo punto a la «E». A esto se le llama DEFLATE.

Al final, una prueba automática le pide al propio Windows que abra cada PNG de Kata y comprueba
que cada píxel es idéntico al original.

## Resultados

Medido con 19 capturas reales: ventanas, páginas, capturas de la Store y una foto con mucho ruido.

| Tipo de captura | Ahorro frente a Windows |
|---|---|
| Ventanas y páginas | 5–37 % (la mayoría, 14–23 %) |
| Pocos colores (iconos, ventanas simples) | 50–65 % |
| Fotos o fondos con mucho ruido | 0 % (igual) |
| **Media de las 19** | **18 %** |
| Peor caso | 0,1 % más grande (prácticamente igual) |

**Velocidad:** tarda unas 1,6 veces lo que Windows, en segundo plano. Una captura típica de
2556×1440 tarda 88 ms (Windows: 64 ms). La foto con ruido de 2560×1440, 498 ms (Windows: 271 ms).

**Una nota sobre cómo leer los números.** Medimos dos cosas: la media del ahorro por captura, y el
ahorro sobre el peso total de todas juntas. La media es la que importa, porque cuenta lo que cada
persona ve en cada captura. El total lo dominan dos fotos grandes y ruidosas que casi no se
pueden comprimir, así que es mucho menor: 5,6 %.

**En el uso diario.** Alex tomó las mismas capturas con Kata, que aún usaba el codificador de
Windows, y con Kata Dev. Comprimiendo los mismos píxeles con los dos:

| Captura | Ahorro frente a Windows |
|---|---|
| Pantalla completa con una ventana (2560×1440) | 8 % |
| Recorte pequeño (680×100) | 5 % |
| Pantalla completa del fondo de pantalla, una foto | −0,5 % (un poco más grande) |

Menos que en el banco, que tenía más capturas de pocos colores. Y con fotos, nuestro método pierde
por poco. De ahí el plan del final.

## Bitácora de experimentos

Cada experimento se midió con las mismas capturas reales, comparando el tamaño y el tiempo contra
el codificador de Windows (WIC), y comprobando que la imagen saliera idéntica.

### 1. La versión de partida

Paleta, filtros elegidos por la regla habitual y DEFLATE con búsquedas de 3 bytes. Funcionó a la
primera y ya ahorraba **13,3 % de media**, pero algunas capturas salían hasta un **4,8 % más
grandes** que con Windows. No era aceptable: nunca queremos un archivo más grande.

### 2. Cómo elegir el filtro de cada fila

Probamos usar siempre el mismo filtro, y tres formas de elegirlo en cada fila. Ahorro sobre el
peso total (primera ronda, antes de las demás mejoras):

| Filtro | Ahorro total |
|---|---|
| Ninguno, siempre | −22,6 % |
| Izquierda, siempre | −3,9 % |
| Arriba, siempre | −2,1 % |
| Paeth (el mejor vecino), siempre | −0,3 % |
| El que deja los bytes más cerca de cero (la regla habitual) | +1,0 % |
| El de menor entropía (el más predecible) | +1,7 % |
| **El que deja menos pares de bytes distintos (nuestra medida)** | **+3,0 %** |

**Decisión:** contar pares. DEFLATE vive de repeticiones, y una fila con pocos pares distintos
repite más. La regla habitual premia los bytes pequeños, que no es lo mismo que repetidos.

### 3. Buscar repeticiones de 4 bytes en vez de 3

En una captura en RGB, 3 bytes son exactamente un píxel. Buscando por 3 bytes, la búsqueda se
llenaba de píxeles sueltos del mismo color, repeticiones tan cortas que cuestan casi lo mismo que
guardar los bytes. Buscando por 4, cada candidato ya ocupa más de un píxel.

| Configuración | Media | Peor caso | Tiempo |
|---|---|---|---|
| Regla habitual, 3 bytes | 13,3 % | −4,8 % | 1297 ms |
| Regla habitual, 4 bytes | 11,6 % | −0,9 % | 967 ms |
| Pares, 3 bytes, sin repeticiones cortas lejanas | 19,1 % | −3,0 % | 1704 ms |
| **Pares, 4 bytes** | **18,3 %** | **−0,1 %** | **1358 ms** |

**Decisión:** pares con 4 bytes. Hay una opción con media un poco mayor (19,1 %), pero puede dejar
archivos 3 % más grandes y es más lenta.

### 4. Descartar repeticiones de 3 bytes lejanas

Una repetición de 3 bytes a más de 4096 bytes de distancia cuesta casi lo mismo que los bytes que
reemplaza. Descartarlas mejoró poco la regla habitual (de 0,6 % a 1,4 % total) y con 4 bytes deja
de importar. **Descartado.**

### 5. Cuánto buscar

En cada posición, Kata revisa hasta cierto número de candidatos anteriores. Más candidatos
encuentran repeticiones más largas, pero tardan más.

| Candidatos | Media | Peor caso | Tiempo |
|---|---|---|---|
| 16 | 16,9 % | −2,1 % | 926 ms |
| 32 | 17,4 % | −1,0 % | 1082 ms |
| **64** | **18,3 %** | **−0,1 %** | **1340 ms** |
| 128 | 18,8 % | +0,9 % | 1654 ms |

**Decisión:** 64. Con 128 se ahorra medio punto más, pero tarda un 23 % más.

### 6. Una tabla de búsqueda más grande

Pasar la tabla de 32 768 a 65 536 entradas no cambió nada medible. **Se queda en 32 768**, que
gasta la mitad de memoria.

### 7. Buscar menos en zonas con ruido

Tras 8 búsquedas seguidas sin éxito, buscar con la cuarta parte de candidatos. Fue un 3 % más
rápido con el mismo tamaño. **Descartado:** no compensa la complejidad.

### 8. Filtros más rápidos

Calcular los cinco filtros píxel a píxel tardaba unos 60 ms en una captura de 2560×1440.
Reescritos como cinco recorridos sencillos, uno por filtro, que el procesador ejecuta de corrido,
bajaron a unos 13 ms. **Adoptado.**

## Datos para nerds

**El archivo:** firma PNG, `IHDR`, `PLTE` (solo con paleta), un único `IDAT` e `IEND`. Sin canal
alfa (las capturas son opacas), sin entrelazado y sin metadatos. Cada bloque lleva su CRC-32.

**Paleta:** se arma en una sola pasada con una tabla hash propia de 512 entradas y un atajo para
el color del píxel anterior. Profundidad de 1, 2, 4 u 8 bits según el número de colores, con
filas empaquetadas desde el bit más alto y sin filtro: los índices son nombres, no cantidades.

**RGB:** 8 bits por canal. Cada fila prueba los cinco filtros (None, Sub, Up, Average, Paeth) y
elige el que deja menos pares de bytes vecinos distintos. Para contarlos sin borrar nada entre
filas, una tabla de 65 536 entradas guarda en qué fila se vio cada par.

**DEFLATE:**

| Parámetro | Valor |
|---|---|
| Ventana | 32 KiB, el máximo de DEFLATE |
| Búsqueda | Cadenas hash sobre 4 bytes, con una tabla de 2¹⁵ entradas |
| Candidatos por posición | 64, o 16 si ya hay una repetición de 8 bytes o más |
| Repetición que corta la búsqueda | 128 bytes |
| Decisión con un byte de espera | Cada repetición espera un byte por si empieza otra más larga, salvo las de 32 bytes o más |
| Bloques | Cada 32 768 símbolos, con códigos de Huffman dinámicos propios |
| Códigos de Huffman | Óptimos con límite de longitud (*package-merge*): 15 bits, 7 para los códigos de longitudes |
| Comparación de bytes | De 8 en 8, en palabras de 64 bits |
| Envoltorio | zlib, con su suma Adler-32 |

**Código:** [`crates/editor/src/png.rs`](../crates/editor/src/png.rs) y
[`crates/editor/src/deflate.rs`](../crates/editor/src/deflate.rs). Unas 900 líneas de Rust puro,
con pruebas, sin ninguna dependencia.

## Plan para ir a fondo (pendiente)

Hasta ahora fuimos prudentes: técnicas clásicas, bien medidas. Lo siguiente es más atrevido y
experimental.

**Metas:**

- **Nunca más grande que Windows**, en ninguna captura, fotos incluidas.
- **Ganar de verdad en el día a día:** más de 15 % en ventanas y páginas, no solo en el banco.
- **Sin hacer esperar:** todo sigue en segundo plano, con la captura ya en el portapapeles.

**Lo que no se puede cambiar.** El archivo sigue siendo un PNG normal. Eso fija las herramientas:
cinco filtros por fila y DEFLATE, que solo recuerda 32 KB hacia atrás (en una captura de 2560 px de
ancho, unas 4 filas). Lo que sí decidimos: el filtro de cada fila, cómo se describen las
repeticiones, dónde empieza cada bloque y cómo se guardan los colores. Con fotos, el margen es
pequeño para cualquier codificador: tienen ruido y casi nada se repite. Ganar mucho ahí exigiría
otro formato, y eso ya lo descartamos.

### Fase 0. Medir mejor

- **Un banco de pruebas permanente:** una prueba en el repo que compara con Windows cada captura
  de una carpeta, con un solo comando. Las capturas son privadas y el repo es público, así que
  viven en `Kata Archivos`.
- **Más variedad:** fondos de pantalla, pantalla completa con la barra de tareas, modo oscuro, 4K,
  texto pequeño y capturas editadas con Desenfocar y Foco, que es lo que de verdad se guarda.

### Fase 1. Nunca más grande

- **Varias recetas a la vez:** comprimir la misma captura con 3 o 4 recetas en paralelo, una por
  núcleo del procesador (filtros por pares, por la regla habitual o por entropía; con o sin
  paleta), y quedarse con la más pequeña. Cuesta procesador, no espera.
- **Filas de foto y filas de ventana:** nuestra medida de pares gana en ventanas, pero no en fotos.
  Elegir la medida según el tipo de fila.

### Fase 2. DEFLATE óptimo

La que más promete, en todo tipo de capturas.

- **Repeticiones por su costo real en bits:** en vez de quedarse con la más larga en cada punto,
  buscar el camino más barato por todo el archivo, y repetirlo unas veces afinando los códigos.
- **Bloques a medida:** cortar donde cambia la imagen (por ejemplo, donde el fondo de pantalla se
  encuentra con la barra de tareas), para que cada bloque tenga sus propios códigos.

### Fase 3. Filtros atrevidos

- **Probar de verdad:** comprimir cada fila con los cinco filtros y quedarse con el que menos bits
  cuesta, en vez de estimarlo.
- **Pensar varias filas por delante:** el filtro de una fila cambia cuánto se le parecen las
  siguientes. Buscar la mejor secuencia, no la mejor fila suelta.

### Fase 4. Colores

- **Escala de grises:** si todos los píxeles son grises, guardar un canal en vez de tres.
- **Paleta ordenada** por uso o por qué colores van juntos, para que los índices formen patrones;
  y paleta con filtros cuando convenga.

### Fase 5. Ideas sin garantía

Probablemente fallen, pero se miden:

- **Buscar arriba primero:** en una captura, lo que más se repite suele estar justo encima.
- **Entrelazado:** el PNG entrelazado casi siempre pesa más, pero cambia qué cabe en esos 32 KB.
  En capturas muy anchas, quizá no.

### Para pagar el tiempo

- **Varios núcleos e instrucciones vectoriales** (SIMD) en los filtros y la búsqueda.
- **Empezar antes:** comprimir mientras el editor está abierto, para que guardar sea instantáneo.

Cada experimento se mide con el banco y se anota en la bitácora, funcione o no. Una idea se queda
solo si mejora la media sin dejar ninguna captura más grande que con Windows.

## Cómo se mide

1. Se toman capturas reales, de distintos tamaños y contenidos.
2. Cada una se guarda con el codificador de Windows y con el de Kata, midiendo tamaño y tiempo.
3. Windows abre el PNG de Kata y se comprueba que cada píxel sea idéntico.
4. Se anotan la media por captura, el peor caso y el tiempo total. Una idea se queda si mejora la
   media sin empeorar de forma notable el peor caso ni la velocidad.
5. Cada experimento se anota en este documento, funcione o no.
