# La compresión PNG de Kata

Kata guarda cada captura como PNG con un compresor propio, escrito desde cero, **sin cambiar un
solo píxel**. Frente al que trae Windows, en el banco de pruebas los archivos pesan **35,5 % menos
de media**: entre 18 y 48 % en ventanas, más de 50 % con pocos colores y entre 8 y 11 % cuando hay
fotos. Ninguna captura sale más grande que con Windows.

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
   Kata parte la captura en franjas de unas pocas filas y prueba cada franja de siete maneras:
   con cada filtro, y eligiendo el filtro fila por fila con dos medidas distintas. Se queda con
   la manera que de verdad pesa menos al comprimirla. Así el fondo de pantalla, la barra de
   tareas y el texto reciben cada uno lo que les conviene.
3. **Comprimir.** Primero busca repeticiones y las cambia por avisos cortos del tipo «repite los
   40 bytes que hubo hace 300». Luego le da códigos cortos a lo que más aparece y largos a lo raro,
   como el código Morse le da un solo punto a la «E». A esto se le llama DEFLATE. Kata no se
   queda con la primera repetición que encuentra: calcula cuántos bits cuesta cada camino posible
   y elige el más barato, tres veces seguidas, afinando los precios en cada vuelta. Y cambia de
   códigos donde cambia la imagen.

Todo esto usa todos los núcleos del procesador a la vez, en segundo plano.

Al final, una prueba automática le pide al propio Windows que abra cada PNG de Kata y comprueba
que cada píxel es idéntico al original.

## Resultados de la primera versión

Los de esta sección son de la primera versión del compresor, con las 19 capturas reales de
entonces. Los de ahora están en [Dónde quedó](#dónde-quedó).

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

### 9. Un banco de pruebas permanente

Las 19 capturas de los primeros experimentos eran reales y privadas, así que no viven en el repo.
Ahora hay una prueba, `weighing_a_folder`, que pesa cada imagen de una carpeta con Windows y con
Kata, comprueba que Windows lea la de Kata píxel a píxel y da la media, el peor caso y el tiempo:

```
$env:KATA_BENCH = "C:\ruta\a\la\carpeta"
cargo test --release -p kata weighing_a_folder -- --ignored --nocapture
```

El banco de ahora son 10 capturas sintéticas, dibujadas con las fuentes y los fondos de pantalla
de Windows: una ventana clara sobre un fondo de foto, un editor de código oscuro, una página de
ajustes con degradado, el escritorio solo, un recorte pequeño, una hoja de cálculo, una página web
larga con fotos, una ventana oscura en 4K, un diálogo de pocos colores y una captura editada con
Foco y Desenfocar. Con ellas, la versión anterior ahorraba **26,7 % de media** (peor caso 2,3 %,
7,2 % del peso total) en 978 ms. Todo lo que sigue se mide contra eso, en un equipo de 12 núcleos.

### 10. Probar de verdad los filtros, por franjas

En vez de estimar qué filtro conviene a cada fila, la captura se parte en franjas de unos 128 KB
y cada una se filtra de siete maneras: los cinco filtros fijos, el filtro por fila con nuestra
medida de pares y el filtro por fila con la regla habitual. Cada manera se comprime rápido (solo
para pesarla) y la franja se queda con la más ligera. Las franjas se reparten entre los núcleos.

Junto con el experimento 11: **28,5 % de media**, peor caso 2,9 %, en 605 ms. **Adoptado.**

Variantes medidas, todas **descartadas**:

| Variante | Media |
|---|---|
| Franjas de 64 KB | 33,6 % (frente a 33,8 %) |
| Franjas de 32 KB | 33,5 % |
| Franjas de 256 KB | 33,4 % |
| Una sola manera para toda la captura | 33,4 %, y tarda un 65 % más |
| Pesar cada franja viendo las filas de encima | 33,8 %: igual, con más código |
| Cobrar un costo por cambiar de manera entre franjas | 33,6 % |

### 11. Todos los núcleos

Encontrar repeticiones solo necesita leer los bytes anteriores, que no cambian. Así que los datos
se parten en tramos de 256 KB y cada núcleo lee uno, mirando hacia atrás en el tramo anterior
como si lo hubiera leído él. No se pierde nada de compresión y el tiempo baja casi tanto como
núcleos hay. Los tramos se procesan por tandas, tantos como núcleos, para no guardar en memoria
más de lo necesario. **Adoptado.**

### 12. El camino más barato en bits

Hasta aquí, en cada punto se tomaba la repetición más larga. Pero la más larga no siempre es la
más barata: a veces conviene una más corta y más cercana, o escribir los bytes tal cual. Ahora
cada tramo se lee primero como antes, y con eso se le pone precio en bits a cada byte y a cada
repetición. Luego se busca, entre todas las maneras de recorrer el tramo, la que suma menos bits.
Con el resultado se afinan los precios y se repite.

| Vueltas | Media | Peor caso | Tiempo |
|---|---|---|---|
| 0 (la repetición más larga) | 28,5 % | 2,9 % | 605 ms |
| 1 | 32,8 % | 7,2 % | 1738 ms |
| 2 | 33,3 % | 7,5 % | 2911 ms |

Es la mejora más grande, también en fotos (de 2,9 a 7,5 %). Pero cada vuelta volvía a buscar las
repeticiones. Buscándolas una sola vez y guardándolas para todas las vueltas:

| Vueltas, con las repeticiones guardadas | Media | Tiempo |
|---|---|---|
| 1 | 31,5 % | 1474 ms |
| 2 | 33,1 % | 1653 ms |
| **3** | **33,5 %** | **1842 ms** |

**Decisión:** tres vueltas. Las repeticiones de 128 bytes o más se toman enteras, sin mirar
dentro: bajar ese umbral a 64 o a 32 ahorra tiempo, pero pierde hasta 2,5 puntos.

### 13. Cuánto buscar, otra vez

| Candidatos por posición | Media | Tiempo |
|---|---|---|
| 16 | 31,7 % | 1444 ms |
| 32 | 32,6 % | 1951 ms |
| **64** | **33,3 %** | **2911 ms** |
| 128 (con tres vueltas) | 33,8 % | 2500 ms |

Probamos también rendirse antes: dejar de buscar tras 8, 16 o 24 candidatos seguidos sin mejora.
Ahorra tiempo, pero pierde lo mismo que buscar menos (31,3 a 32,6 %). **Descartado:** se queda en
64. Lo que sí quedó es descartar cada candidato comparando cuatro bytes en vez de uno antes de
medirlo: el mismo resultado, un 6 % más rápido.

### 14. Bloques a medida

Antes, cada bloque tenía 32 768 símbolos. Ahora los símbolos llegan en trozos pequeños, y un trozo
se une al bloque en curso salvo que los dos pesen menos por separado, cada uno con sus códigos.
Así los códigos cambian donde cambia la imagen.

| Tamaño del trozo | Media | Total |
|---|---|---|
| 4096 símbolos | 32,5 % | 12,7 % |
| 1024 | 32,8 % | 12,7 % |
| 512 | 33,7 % | 13,6 % |
| **256** | **33,8 %** | **13,8 %** |

**Decisión:** 256. Además, un bloque tan pequeño que no paga sus propios códigos usa los códigos
fijos de DEFLATE: en un recorte pequeño ahorra unas decenas de bytes.

### 15. Precios con los códigos de verdad

El camino más barato se busca con precios estimados (la entropía de cada símbolo). Probamos
ponerle los precios reales: el largo del código de Huffman que tendría cada símbolo.

| Precios | Media | Tiempo |
|---|---|---|
| Estimados, 3 vueltas | 33,8 % | 1909 ms |
| Reales desde la 2.ª vuelta, 3 vueltas | 33,7 % | 1948 ms |
| Reales desde la 3.ª, 4 vueltas | 33,8 % | 2145 ms |
| Estimados, 5 vueltas | 34,0 % | 2334 ms |

**Descartado.** Los precios reales son enteros y pierden el matiz de los estimados. Más vueltas
ganan dos décimas y cuestan un 22 % más de tiempo.

### 16. Buscar menos dentro de una repetición buena

Si en una posición ya hay una repetición larga, en la siguiente sigue valiendo, un byte más corta.
Probamos buscar ahí con menos candidatos y reutilizar esa.

| Umbral, candidatos | Media | Tiempo |
|---|---|---|
| Sin esto | 33,8 % | 2067 ms |
| 16 bytes, la octava parte | 33,4 % | 1817 ms |
| 8 bytes, la cuarta parte | 33,1 % | 1685 ms |
| 4 bytes, la octava parte | 31,9 % | 1377 ms |

**Descartado.** Es la misma cuenta de siempre: cada candidato que se deja de mirar se paga en
tamaño. Ganar las dos cosas pide mirar mejor, no menos.

### 17. La fila de arriba

Una idea propia de las capturas. La búsqueda mira los 64 lugares más cercanos que empiezan igual;
donde mucho se parece (un fondo liso, una tabla, un degradado), esos 64 quedan todos a pocos bytes
y nunca llega a la fila de arriba, que está miles de bytes atrás y es donde de verdad se repite
la imagen. Ahora, en cada posición, además de la búsqueda, se compara directamente con la fila de
arriba, y si repite más que lo encontrado, es un candidato más. Cuesta una comparación.

| Variante | Media | Peor caso | Tiempo |
|---|---|---|---|
| Sin la fila de arriba | 33,8 % | 8,2 % | 1670 ms |
| Tomarla entera cuando es larga, sin buscar | 26,6 % | −4,3 % | 1663 ms |
| **Como un candidato más** | **35,3 %** | **8,2 %** | **1658 ms** |
| También dos filas arriba | 35,3 % | 8,2 % | 1887 ms |
| También hasta cuatro | 35,3 % | 8,1 % | 1738 ms |

**Adoptado**, como un candidato más. Tomarla entera sin buscar fue un error instructivo: en un
fondo liso, repetir el byte de al lado cuesta muchos menos bits que repetir la fila de arriba,
aunque sea igual de larga. Con ella, bajar la búsqueda a 32 candidatos da 34,8 % en 1452 ms: más
compresión que antes y un 13 % más rápido; nos quedamos con 64.

### 18. Pesar mejor cada franja

Para elegir cómo filtrar cada franja se comprime rápido, con 8 candidatos por posición. Con la fila
de arriba ya incluida:

| Candidatos al pesar | Media | Tiempo |
|---|---|---|
| 8 | 35,3 % | 1658 ms |
| **16** | **35,5 %** | **1664 ms** |
| 32 | 35,5 % | 1721 ms |

**Decisión:** 16.

### Dónde quedó

| Captura | Windows | Antes | Ahora | Ahorro | Tiempo |
|---|---|---|---|---|---|
| Editor de código oscuro (2560×1440) | 188 693 | 121 798 | 103 290 | 45,3 % | 109 ms |
| Recorte pequeño (680×100) | 5 717 | 3 018 | 2 772 | 51,5 % | 5 ms |
| Escritorio con foto (2560×1440) | 3 406 662 | 3 327 329 | 3 127 001 | 8,2 % | 614 ms |
| Diálogo de pocos colores (480×320) | 10 426 | 4 532 | 4 132 | 60,4 % | 6 ms |
| Captura editada (1920×1080) | 163 766 | 135 995 | 112 541 | 31,3 % | 104 ms |
| Ajustes con degradado (1920×1080) | 69 737 | 62 092 | 52 066 | 25,3 % | 59 ms |
| Hoja de cálculo (1600×900) | 74 223 | 39 256 | 33 001 | 55,5 % | 47 ms |
| Página web larga con fotos (1920×2400) | 1 353 874 | 1 305 386 | 1 203 838 | 11,1 % | 378 ms |
| Ventana oscura en 4K (3840×2160) | 330 364 | 196 632 | 172 151 | 47,9 % | 105 ms |
| Ventana clara sobre foto (2560×1440) | 502 807 | 472 549 | 412 075 | 18,0 % | 238 ms |
| **Media** | | 26,7 % | | **35,5 %** | |

En bytes. El peor caso pasó de 2,3 a 8,2 %, y el ahorro sobre el peso total, de 7,2 a 14,5 %. El
banco entero tarda 1664 ms, frente a 978 ms: 1,7 veces más, en segundo plano y con la captura ya
en el portapapeles.

## Datos para nerds

**El archivo:** firma PNG, `IHDR`, `PLTE` (solo con paleta), un único `IDAT` e `IEND`. Sin canal
alfa (las capturas son opacas), sin entrelazado y sin metadatos. Cada bloque lleva su CRC-32.

**Paleta:** se arma en una sola pasada con una tabla hash propia de 512 entradas y un atajo para
el color del píxel anterior. Profundidad de 1, 2, 4 u 8 bits según el número de colores, con
filas empaquetadas desde el bit más alto y sin filtro: los índices son nombres, no cantidades.

**RGB:** 8 bits por canal, en franjas de filas que suman unos 128 KB. Cada franja se filtra de
siete maneras (None, Sub, Up, Average o Paeth en todas sus filas; o, fila por fila, el filtro que
deja menos pares de bytes vecinos distintos, o el que deja los bytes más cerca de cero) y se queda
con la que menos pesa tras una compresión rápida, de 16 candidatos por posición más la fila de
arriba, y sin afinar.

**DEFLATE:**

| Parámetro | Valor |
|---|---|
| Ventana | 32 KiB, el máximo de DEFLATE |
| Tramos | De 256 KiB, uno por núcleo a la vez; cada uno ve los 32 KiB anteriores |
| Búsqueda | Cadenas hash sobre 4 bytes, con una tabla de 2¹⁵ entradas |
| Candidatos por posición | 64, más la fila de arriba; se guarda, para cada largo, la repetición más cercana |
| Repetición que se toma entera | 128 bytes o más |
| Lectura | El camino de menos bits por el tramo, tres veces: la primera con los precios de tomar siempre la repetición más larga, y cada una con los de la anterior |
| Precios | La entropía de cada símbolo, en 1/32 de bit, entre 1 y 15 bits |
| Bloques | Trozos de 256 símbolos que se unen mientras pesen menos juntos; 262 144 símbolos como mucho |
| Códigos de Huffman | Óptimos con límite de longitud (*package-merge*): 15 bits, 7 para los códigos de longitudes; o los fijos de DEFLATE si el bloque pesa menos con ellos |
| Comparación de bytes | De 8 en 8, en palabras de 64 bits |
| Envoltorio | zlib, con su suma Adler-32 |

**Código:** [`crates/editor/src/png.rs`](../crates/editor/src/png.rs),
[`crates/editor/src/deflate.rs`](../crates/editor/src/deflate.rs) y
[`crates/editor/src/parallel.rs`](../crates/editor/src/parallel.rs). Unas 1400 líneas de Rust
puro, con pruebas, sin ninguna dependencia.

## Lo que sigue

Del plan para ir a fondo ya está hecho lo que más prometía: el banco permanente, probar los
filtros de verdad, el camino más barato en bits, los bloques a medida y usar todos los núcleos.
**Lo que no se puede cambiar** sigue igual: el archivo es un PNG normal, sin pérdida. Eso fija las
herramientas: cinco filtros por fila y DEFLATE, que solo recuerda 32 KB hacia atrás. Quedan estas
ideas, de más a menos prometedoras:

- **Un buscador de repeticiones mejor.** Buscar más hondo sigue dando,
  pero cuesta tiempo. Una estructura que encuentre la repetición más larga sin recorrer
  candidatos uno a uno daría eso y más, por menos.
- **Filtros fila por fila, probados de verdad.** Hoy se prueban por franjas. Probar cada fila con
  lo que ya se eligió encima es más fino y más caro.
- **Colores:** paleta con filtros cuando convenga, índices de 8 bits frente a empaquetados, y
  grises con filtros frente a la paleta.
- **Empezar antes:** comprimir mientras el editor está abierto, para que guardar sea instantáneo.
- **Más candidatos propios de una captura:** la fila de arriba funcionó; quedan por probar el
  píxel de al lado en diagonal y la misma columna de otra ventana igual.
- **Ideas sin garantía:** el entrelazado en capturas muy anchas.

Cada experimento se mide con el banco y se anota en la bitácora, funcione o no. Una idea se queda
solo si mejora la media sin dejar ninguna captura más grande que con Windows.

## Cómo se mide

1. Se toman capturas reales, de distintos tamaños y contenidos.
2. Cada una se guarda con el codificador de Windows y con el de Kata, midiendo tamaño y tiempo.
3. Windows abre el PNG de Kata y se comprueba que cada píxel sea idéntico.
4. Se anotan la media por captura, el peor caso y el tiempo total. Una idea se queda si mejora la
   media sin empeorar de forma notable el peor caso ni la velocidad.
5. Cada experimento se anota en este documento, funcione o no.

La bienvenida hace esta misma cuenta con la primera captura de quien la usa: la codifica también con el PNG de Windows, en memoria y en segundo plano, y dice en Lista cuánto menos pesa con Kata.
