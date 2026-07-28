# Modo Hub: personal y equipo — análisis de diseño

> ⚠️ **Esto NO está implementado.** Es el análisis previo, escrito el
> 2026-07-28, de cómo funcionaría el Hub y qué problemas aparecen al usarlo
> entre varias personas. Sirve para decidir qué construir y en qué orden.
> Nada de lo que aquí se describe existe todavía en MichiClaude.

---

## 1. Qué problema resuelve el Hub (y cuál no)

MichiClaude tiene dos mitades y **el Hub solo afecta a una**.

| mitad | de dónde sale | ¿la arregla el Hub? |
| --- | --- | --- |
| **Cuota** (sesión 5 h, límites semanales) | del servidor de Anthropic | **No.** Ya es global: el 13% es el 13% desde cualquier máquina |
| **Gasto por proyecto, tendencia, modelos** | de los registros locales `~/.claude` de cada máquina | **Sí.** Es lo que hoy está partido en pedazos |

Tampoco puede capturar los chats de **claude.ai web**: no dejan registro
local en ninguna máquina. Para eso está la fila "claude.ai / otros", que los
estima por diferencia.

### El ejemplo que lo explica

Trabajas el proyecto `sparky-site` desde tres sitios:

| máquina | gasto |
| --- | --- |
| Laptop de casa | $30 |
| VPS | $50 |
| PC del trabajo | $20 |

Lo real son **$100**. Hoy verías:

| desde dónde miras | ves | te falta |
| --- | --- | --- |
| Laptop | $80 (laptop + VPS) | los $20 del trabajo |
| PC del trabajo | $70 (trabajo + VPS) | los $30 de la laptop |

Ninguna dice la verdad y encima se contradicen. Con el Hub, las dos dicen
$100.

### Por qué pasa hoy

El VPS es una **fuente** que tú consultas: tu Windows le pregunta "dame tus
datos". Pero el VPS no sabe nada de tu Windows, y tus máquinas no se conocen
entre ellas.

El Hub cambia el papel del servidor: deja de ser una fuente y pasa a ser el
**punto de encuentro**. Cada máquina deja ahí su resumen y todas leen el de
todas.

### Cuándo NO aporta

- Si usas Claude Code en **una sola máquina**.
- Si ya tienes **una máquina que lee a las demás** por SSH y solo miras desde
  ahí. Ese es el caso de "un Windows + un VPS": el Hub no cambia nada
  visible. Empieza a pagar **con la tercera máquina**, o cuando quieres mirar
  desde otro sitio.
- Si tus máquinas **no comparten un servidor** al que todas lleguen.

---

## 2. Cómo funciona por dentro

Esta es la pieza que explica casi todo lo demás.

Cada máquina deja en el servidor **un archivo con un resumen**, en
`~/.michiclaude/hosts/<nombre>.json`, y ese archivo **se sobreescribe entero**
en cada ciclo. No se acumula historia: es una foto de "esto es lo que tengo
ahora".

**La foto incluye la serie diaria de los últimos 30 días** — la misma que
pinta la gráfica de tendencia. Así que la historia sí viaja al hub.

De ahí sale la regla mecánica:

> **Lo que no esté en la foto siguiente, deja de existir en el hub.**

---

## 3. Qué se comparte: el interruptor va en cada servidor

El error de diseño más fácil de cometer aquí es poner un único interruptor
global en la app. No sirve, porque una misma persona puede tener **dos
servidores con reglas distintas**:

| servidor | qué subo ahí |
| --- | --- |
| `mi-vps` (tuyo) | **Todo** — es tuyo, no te escondes nada a ti mismo |
| `rutalibre` (del equipo) | **Solo el proyecto compartido** |

Por eso la pregunta vive **en cada servidor**, no en la app.

### El formulario al agregar un servidor

```
AGREGAR SERVIDOR
Nombre corto:  rutalibre
Host SSH:      oscar@10.0.0.5
Nombre de esta máquina:  windows-oscar

¿Qué compartes con este servidor?
○ Todo — es mi servidor
● Solo los proyectos que marque    ← preseleccionado
   ☐ michiclaude
   ☐ sparky-site
   ☐ tesis
   ☐ rutalibre
```

### Dos reglas de seguridad, y por qué

**1. Se filtra en el ORIGEN, no en la pantalla.** La casilla decide *qué se
sube*, no *qué se muestra*. Lo que no compartes **nunca sale de tu PC**.

Es la diferencia entre "los demás no lo ven" y "los demás no lo tienen". Solo
la segunda aguanta: cualquiera con acceso al servidor podría leer los
archivos directamente. Ocultar en la interfaz sería seguridad de mentira.

**2. Las casillas nacen APAGADAS.** Aunque agregues el servidor sin mirar, no
se sube nada. Y los proyectos que crees después también nacen apagados.

El motivo es la asimetría del error: equivocarte hacia "todo" pone tus
proyectos en el servidor de otra gente y **ya no los puedes retirar**.
Equivocarte hacia "nada" solo hace que veas menos números hasta que te des
cuenta. Uno se arregla, el otro no.

---

## 4. Ejemplo completo: 4 personas, un proyecto

Cuatro amigos construyen `rutalibre`, cada uno desde su PC.

| persona | su máquina | en `rutalibre` | además, suyo personal |
| --- | --- | --- | --- |
| Oscar | `windows-oscar` | $42.10 | — |
| Karla | `kar-lap` | $63.40 | `tesis-karla` $80.00 |
| Dani | `mac-dani` | $18.75 | — |
| Beto | `beto-pc` | $9.25 | `bot-discord` $6.50 |

Lo real del proyecto son **$133.50**.

### Hoy, sin Hub

```
Oscar ve:    rutalibre  local   $42.10
Karla ve:    rutalibre  local   $63.40
Dani ve:     rutalibre  local   $18.75
```

Cuatro personas, cuatro cifras, ninguna es la del proyecto. Si alguien
pregunta "¿cuánto llevamos gastado?", nadie puede responder.

### Con Hub, etiquetando por origen (lo que saldría sin pensarlo)

```
GASTO POR PROYECTO · 7d
rutalibre · kar-lap          $63.40
rutalibre · windows-oscar    $42.10
tesis-karla · kar-lap        $80.00
rutalibre · mac-dani         $18.75
rutalibre · beto-pc           $9.25
bot-discord · beto-pc         $6.50
```

Se ve todo, pero **sigue sin responder la pregunta**.

### Con Hub, fusionando por proyecto (lo que se quiere)

```
GASTO POR PROYECTO · 7d
rutalibre                   $133.50   ← al pasar el mouse:
tesis-karla                  $80.00      kar-lap        $63.40
bot-discord                   $6.50      windows-oscar  $42.10
                                         mac-dani       $18.75
                                         beto-pc         $9.25
```

Una línea por proyecto con el total y el desglose por máquina al pasar el
mouse — **el mismo mecanismo que ya existe** para el desglose por modelo.

### Lo mismo, pero con las casillas puestas

Si Oscar solo comparte `rutalibre` con el servidor del equipo:

**Panel de Oscar** (ve todo, porque sus dos servidores le informan):

```
tesis                    $80.00     ← solo suyo
rutalibre               $133.50
sparky-site              $97.68
michiclaude              $10.37
```

**Panel de Karla** (de Oscar solo recibe lo que marcó):

```
tesis-karla              $80.00     ← lo suyo, que Oscar tampoco ve
rutalibre               $133.50
```

De Oscar, en la pantalla de Karla, solo existe la línea
`windows-oscar $42.10` dentro de `rutalibre`. Sus otros proyectos **nunca
salieron de su PC**.

### Lo que NO se suma: la cuota

El panel de Oscar sigue mostrando **su** 13% de sesión. El de Karla, el suyo.
No se suman ni deben: cada uno tiene su cuenta y su límite.

Conviven en la misma pantalla **dinero del grupo** y **cuota personal**. Son
cosas distintas y ahí está el primer riesgo de confusión.

---

## 5. Ciclo de vida: qué pasa cuando alguien entra, sale o desaparece

### La asimetría que hay que entender

> **Dejar de subir conserva tu historia. Desmarcar la retira.**

Tiene lógica: desmarcar es un acto deliberado de "quiero sacar mis datos de
aquí". Irse de vacaciones no lo es.

### Caso A — Compartí un proyecto por error

Desmarcas. La siguiente foto ya no lo incluye y desaparece para todos.

**Solución:** subir la foto nueva **al instante**, sin esperar al ciclo de
3 minutos. Quien desmarca por error quiere que se vaya ya.

**Lo que no se puede prometer:** estuvo ahí el tiempo que estuvo. Si alguien
lo copió en ese rato, no hay vuelta atrás. Por eso las casillas nacen
apagadas: para que ese rato no exista.

### Caso B — Me sacan del proyecto y desmarco

Mi aporte desaparece del hub, **pasado incluido**:

```
Antes:                        Después:
rutalibre        $133.50      rutalibre         $91.40
  kar-lap         $63.40        kar-lap          $63.40
  windows-oscar   $42.10        mac-dani         $18.75
  mac-dani        $18.75        beto-pc           $9.25
  beto-pc          $9.25
```

El total del equipo baja $42.10 y parece que el proyecto costó menos de lo
que costó. Karla abre su panel al día siguiente y el número cambió sin que
nadie tocara nada.

**Decisión tomada:** tus datos son tuyos y se van contigo. Es coherente con
lo que ya es esta app — sin base de datos propia ni historial largo, a
propósito, para que no haya nada que se pueda perder ni quedarse donde no
debe. Si el equipo quiere contabilidad del proyecto, eso es otra herramienta.

**Pero el cambio no puede ser invisible.** Al lado del total, cuántas
máquinas reportan:

```
rutalibre        $91.40   · 3 máquinas
```

Un número que cambia solo y sin explicación es lo que hace que la gente deje
de confiar en un medidor.

### Caso C — Alguien deja de usar MichiClaude (se fue, o vacaciones)

No desmarca nada: simplemente **deja de subir fotos**. Su archivo se queda
congelado y **su historia sigue visible**:

```
TENDENCIA DIARIA
        ▁▃▅▆▅▆  ← hasta el 26, con los 4
              ▃▄▃  ← del 27 en adelante, con 3
```

**No borrar automáticamente.** La app **no puede distinguir** entre "se fue
del proyecto", "está de vacaciones dos semanas" y "se le descompuso la PC".
Si borra sola, quien vuelva de vacaciones descubre que lo desaparecieron.

Es la misma regla que ya rige los globos de aviso: *un reloj no sabe si el
usuario estaba delante*.

**Lo que sí se hace:** mostrar la edad de cada máquina.

```
rutalibre        $133.50   · 4 máquinas
  kar-lap         $63.40   hace 5 min
  windows-oscar   $42.10   hace 5 min
  mac-dani        $18.75   hace 2 h
  beto-pc          $9.25   hace 12 días   ⚠
```

Ese `hace 12 días` responde la pregunta sin que la app adivine nada. Y un
**botón de borrar la máquina**, decidido por una persona, no por un
temporizador.

**Detalle práctico:** borrar el archivo solo funciona si de verdad dejó de
usarlo. Si sigue trabajando, en su siguiente ciclo lo vuelve a subir y
reaparece. Y eso está bien: significa que sigue ahí.

### Caso D — Vacaciones largas

Su actividad se ve hasta el día que paró, el equipo sigue sumando sin él, y
al volver su máquina retoma. **No se pierde nada**, porque sus registros
locales siguen intactos.

**Matiz:** la ventana es de 30 días. Si se va **más de un mes**, al volver su
propia máquina ya no tiene esos días viejos que contar y caen solos. No es un
fallo: es el tamaño de la ventana.

### Caso E — Borrar el archivo de otro

Borra **todo**, pasado incluido. Su línea desaparece de la gráfica como si
nunca hubiera trabajado.

**Solución:** el botón avisa de lo que hace — *"esto borra también su
historial de los últimos 30 días"*. No es lo mismo que ocultar una fila.

### Caso F — La PC se formatea (la trampa)

MichiClaude calcula leyendo `~/.claude`. **Si formatean, esos registros ya no
existen.** Al reinstalar, esa persona empieza de cero.

| al volver a registrarse… | qué pasa |
| --- | --- |
| **con el mismo nombre** | su foto nueva (casi vacía) **sobreescribe la vieja** y se pierde la historia que estaba a salvo en el hub |
| **con otro nombre** | la vieja se queda congelada y la nueva empieza aparte: se conserva todo, pero aparece como dos máquinas |

Ninguna es obviamente correcta.

**Solución:** avisar al conectar — *"ya existe una máquina llamada `beto-pc`
en este servidor. Si continúas, reemplazarás sus datos."* Que la persona
decida sabiendo. El mismo aviso cubre el choque de nombres entre dos personas
distintas.

---

## 6. "Quiero ver dónde bajó el costo y por qué"

Choca de frente con "cada quien controla sus datos". Si alguien puede retirar
su información, el equipo no puede tener un registro que nadie pueda tocar.
**No se pueden tener las dos cosas.**

**Salida:** un **registro de eventos** en el hub, de una línea:

```
27/Jul  beto-pc dejó de reportar
28/Jul  ana-mac se unió
```

Explica el escalón de la gráfica **sin conservar ni un dólar** de quien se
fue. Metadato, no datos. Barato y honesto.

---

## 7. Permisos: por qué un "modo administrador" en la app sería decorativo

El hub es una carpeta en un servidor. Cualquiera con SSH a ese servidor puede
borrar cualquier archivo con un `rm`, le pongas el botón que le pongas a
MichiClaude. **Un candado que se salta escribiendo un comando no es un
candado.**

Lo que sí funciona son los permisos del propio Linux:

- cada persona con **su usuario**,
- cada archivo de **su dueño**,
- la carpeta configurada para que los demás **lean pero no escriban**.

Ahí nadie puede borrar los datos de otro ni a mano, y el "administrador" es
quien tenga sudo en ese servidor. Es gratis y es real.

**Contrapartida:** con esos permisos, el botón de "borrar máquina parada"
solo puede borrar la tuya. Retirar la de alguien que se fue exige un admin
con acceso al servidor.

---

## 8. Lo que este análisis deja claro

Repasando lo que hizo falta para el caso de equipo:

- cuentas de usuario por persona
- permisos de carpeta
- un rol de administrador
- un registro de eventos
- avisos de reemplazo de datos
- casillas de qué se comparte, por servidor
- separar "mi gasto" de "gasto del equipo" en el pie de totales

**Ese es el motivo por el que el proyecto decidió no hacer modo equipo.** No
es que sea imposible: es que es otro producto, con otro público y otros
problemas.

### Decisiones que quedan abiertas

1. **El pie de totales.** Con gente conectada, ¿HOY y 7D muestran tu gasto o
   el del grupo? Cuando todas las máquinas son tuyas la suma **es** tu gasto;
   con más gente **no lo es**, y ponerlo ahí sería mentir.
2. **Los dólares son estimados** (equivalente-API). Entre amigos, "$133.50"
   puede leerse como plata que alguien debe. La app ya lo etiqueta, pero en
   grupo conviene que se note más.

---

## 9. Recomendación

**Hub personal primero.** En ese caso nada de lo anterior aplica: el dueño de
todas las máquinas es uno mismo y no hay de quién protegerse.

Tres fases, cada una verificable con un Windows y un VPS:

1. **Subir** — cada meter deja su resumen en el servidor. Se comprueba
   mirando que el archivo aparezca.
2. **Fusionar** — el exportador devuelve los resúmenes de los demás,
   excluyendo al que pregunta (`--exclude-host`). Se prueba creando a mano un
   `hosts/otra-maquina.json` con datos plausibles: verifica lo difícil (que
   fusione bien, que no cuente dos veces al que pregunta, qué pasa con un
   host viejo) sin necesitar hardware.
3. **Configuración compartida** — ya con la tubería probada, es guardar y
   leer un JSON más: servidores, alarmas, presupuesto, idioma y tema. Llegar
   a una PC nueva, escribir dos campos y tenerlo todo.

**El interruptor de "qué subo" debe existir desde el diseño**, aunque en la
fase 1 se quede siempre en "todo". Afecta a cómo se construye la subida y
rehacerlo después cuesta más.

### Sobre la fase 3, un límite honesto

Para bajar la configuración del servidor, la PC nueva necesita saber **cuál
es el servidor** — y eso es parte de la configuración. El "cero
configuración" no es alcanzable. Lo realista: instalas, escribes **nombre y
host**, y el resto se hereda. De "reconfigurar todo" a "dos campos".

Y un requisito que MichiClaude no puede resolver: esa PC necesita **tu llave
SSH** para entrar al servidor. Sin ella no hay nada que bajar.

---

## 10. ¿Es un problema real o de nicho?

Real, pero de nicho. La evidencia, revisada el 2026-07-28:

- En **ccusage** —la herramienta de referencia para costos de Claude Code—
  se pidió exactamente esto ([issue #222](https://github.com/ryoppippi/ccusage/issues/222)),
  y el mantenedor lo **cerró como "not planned"** con muy poca participación
  en el hilo.
- Aun así hay quien se lo construyó: **claude-telemetry** pone un agente en
  cada PC que llama a ccusage cada 15 minutos y sincroniza a una base de
  datos; **[claude-usage-tracker](https://github.com/jimdawdy-hub/claude-usage-tracker)**
  lleva "multi-device sync" como titular.
- Hay hasta un artículo titulado *[How I Track Claude Code Costs Across
  Multiple PCs](https://dev.to/ryantech00/how-i-track-claude-code-costs-across-multiple-pcs-13bl)*.
- Para equipos, **Anthropic ya ofrece analíticas nativas** a organizaciones
  ([Claude Code usage analytics](https://support.claude.com/en/articles/12157520-claude-code-usage-analytics)).
  Competir ahí es pelear contra el fabricante.

**Lectura:** cuando alguien escribe un agente y un artículo para resolver
algo, es que le duele de verdad. Pero son minoría: la mayoría usa Claude Code
en una sola máquina.

Dos cosas juegan a favor: los que sí tienen el problema son los **usuarios
pesados** —laptop + servidor, o PC personal + del trabajo—, justo quienes más
se preocupan por su cuota. Y todos los que lo resolvieron son CLI o
dashboards web: **ninguno es un widget de bandeja**.
