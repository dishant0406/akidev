@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)

package live.akidev.tv

import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.foundation.relocation.BringIntoViewResponder
import androidx.compose.foundation.relocation.bringIntoViewResponder
import androidx.compose.ui.geometry.Rect
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.border
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.ui.graphics.Brush
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.BoxScope
import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.zIndex
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalDensity
import androidx.tv.material3.Text
import coil3.compose.AsyncImage
import uniffi.akitv.Title

private val ButtonShape = RoundedCornerShape(8.dp)
private val TileShape = RoundedCornerShape(12.dp)
private val TipShape = RoundedCornerShape(16.dp)

/**
 * A focused control gets the amber ring and grows a little. Everything the remote can land on uses this.
 * It is built from the plain foundation pieces on purpose: the TV library's `Surface` did the same job but cost much more to compose
 * for each poster of a row, and scrolling on the TV was slow (see docs/design.md).
 */
@Composable
fun Focusable(onClick: () -> Unit, shape: Shape, color: Color, modifier: Modifier = Modifier, content: @Composable BoxScope.() -> Unit) {
    var focused by remember { mutableStateOf(false) }
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    // It grows a little when the remote lands on it, and dips a little when OK is pressed, as a button does under a finger.
    val scale by animateFloatAsState(if (pressed) 0.97f else if (focused) 1.05f else 1f, Motion.go(if (pressed) 120 else 240), label = "focus")
    val ring by animateColorAsState(if (focused) Ink.Accent else Color.Transparent, Motion.go(200), label = "ring")
    Box(
        modifier
            .onFocusChanged { focused = it.isFocused }
            .graphicsLayer { scaleX = scale; scaleY = scale }
            .background(color, shape)
            .border(3.dp, ring, shape)
            .clip(shape)
            .clickable(interactionSource = source, indication = null, onClick = onClick),
        content = content,
    )
}

/** The three buttons of the design plan: primary (warm white), secondary (hairline), and plain text. */
enum class Look { Primary, Secondary, Text }

/**
 * The three buttons of the design plan. With an `icon` the button is just the icon, and its `label` shows in a small tip beside it
 * while the remote is on it: the tip floats over the screen, so nothing around the button moves. `labelAlways` keeps the text in the
 * button (for a label that carries information, such as a time). `tipAbove` puts the tip over the button, for one at the foot of the screen.
 */
@Composable
fun AkiButton(
    label: String,
    look: Look,
    modifier: Modifier = Modifier,
    icon: String? = null,
    labelAlways: Boolean = icon == null,
    wide: Boolean = false,
    autofocus: Boolean = false,
    tipAbove: Boolean = false,
    onClick: () -> Unit,
) {
    val fill = when (look) {
        Look.Primary -> Ink.Text
        Look.Secondary -> Ink.Raised
        Look.Text -> Color.Transparent
    }
    val ink = if (look == Look.Primary) Ink.Canvas else Ink.Text
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    if (autofocus) LaunchedEffect(Unit) { focus.requestFocus() }
    val iconOnly = icon != null && !labelAlways
    Box(Modifier.zIndex(if (focused) 1f else 0f).then(if (wide) Modifier.fillMaxWidth() else Modifier)) {
        Focusable(onClick, ButtonShape, fill, modifier.focusRequester(focus).onFocusChanged { focused = it.isFocused }.then(if (wide) Modifier.fillMaxWidth() else Modifier)) {
            Row(
                Modifier.then(if (wide) Modifier.fillMaxWidth() else Modifier).padding(horizontal = if (iconOnly) 14.dp else 20.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                if (icon != null) AkiIcon(icon, tint = ink)
                if (!iconOnly) Text(label, style = Type.Label.copy(color = ink), maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (iconOnly) Tip(label, focused, tipAbove)
    }
}

/** A small label that floats under (or over) the control it belongs to while `visible`. It takes no room, so nothing moves. */
@Composable
fun BoxScope.Tip(label: String, visible: Boolean, above: Boolean = false) {
    Box(Modifier.align(Alignment.Center).size(0.dp).wrapContentSize(unbounded = true).offset(y = if (above) (-52).dp else 52.dp)) {
        AnimatedVisibility(visible, enter = fadeIn(Motion.go(180)) + scaleIn(Motion.go(180), initialScale = 0.94f), exit = fadeOut(Motion.go(110))) {
            Text(
                label, style = Type.Label, maxLines = 1, softWrap = false,
                modifier = Modifier.background(Ink.Raised, TipShape).border(1.dp, Ink.Line, TipShape).padding(horizontal = 14.dp, vertical = 6.dp),
            )
        }
    }
}

/** A stand-in for something that is still loading: a plain shape that breathes a little, in the size of what will come. */
fun Modifier.placeholder(shape: Shape): Modifier = composed {
    val breath by rememberInfiniteTransition(label = "placeholder").animateFloat(0.35f, 0.7f, infiniteRepeatable(tween(Motion.ms(1000)), RepeatMode.Reverse), label = "breath")
    // The colour is read in the draw step, so the breathing does not compose anything again.
    clip(shape).drawBehind { drawRect(Ink.Raised.copy(alpha = breath)) }
}

/**
 * A poster, 2:3. `onFocus` tells the screen which title the remote is on (the home hero follows it).
 * `showName` puts the name under the poster: the grid of a search needs it, the home screen does not (its hero names the title).
 */
@Composable
fun PosterCard(title: Title, showName: Boolean = true, autofocus: Boolean = false, onFocus: () -> Unit = {}, onClick: () -> Unit) {
    val focus = remember { FocusRequester() }
    if (autofocus) LaunchedEffect(Unit) { focus.requestFocus() }
    // With the name under the poster, scrolling to the poster must leave room for the name as well.
    Column(Modifier.width(Poster.width).then(if (showName) Modifier.roomForName() else Modifier), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Focusable(onClick, TileShape, Ink.Surface, Modifier.size(Poster.width, Poster.height).focusRequester(focus).onFocusChanged { if (it.isFocused) onFocus() }) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                // The name sits under the picture, so a title with no picture (or one that fails to load) still reads.
                Text(title.name, style = Type.Meta, textAlign = TextAlign.Center, modifier = Modifier.padding(12.dp))
                AsyncImage(
                    model = title.poster,
                    contentDescription = title.name,
                    contentScale = ContentScale.Crop,
                    onSuccess = { Perf.once("first poster") },
                    modifier = Modifier.fillMaxSize().clip(TileShape),
                )
            }
        }
        if (showName) Text(title.name, style = Type.Body, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 4.dp))
    }
}

object Poster {
    val width = 128.dp
    val height = 192.dp
}

/** A title card for the quiet moments: loading, nothing found, or a failure. An icon, a line, a smaller line, then the actions. */
@Composable
fun StateCard(headline: String, detail: String? = null, icon: String? = null, actions: @Composable () -> Unit = {}) {
    Box(Modifier.fillMaxSize().background(Ink.Canvas), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp), modifier = Modifier.width(720.dp)) {
            if (icon == null) Spinner() else AkiIcon(icon, size = 48.dp, tint = if (icon == "warning") Ink.Danger else Ink.TextDim)
            if (headline.isNotEmpty()) Text(headline, style = Type.Display, textAlign = TextAlign.Center)
            if (detail != null) Text(detail, style = Type.Body, textAlign = TextAlign.Center)
            Box(Modifier.padding(top = 12.dp)) { actions() }
        }
    }
}

/** A one-line text field. The amber border shows the remote is on it; OK opens the keyboard. */
@Composable
fun Field(
    value: String,
    onChange: (String) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier,
    secret: Boolean = false,
    action: ImeAction = ImeAction.Done,
    onAction: () -> Unit = {},
    icon: String? = null,
) {
    // Resting, the field is a plain box the remote can land on: the keyboard stays shut. OK on it starts editing (the real text field takes
    // the focus and the keyboard opens). When the remote moves away, it rests again. Without this the keyboard opened by itself whenever
    // the remote passed over the field.
    var editing by remember { mutableStateOf(false) }
    var focused by remember { mutableStateOf(false) }
    var hadFocus by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current
    LaunchedEffect(editing) { if (editing) focus.requestFocus() else hadFocus = false }
    val frame = modifier
        .border(if (focused) 3.dp else 2.dp, if (focused) Ink.Accent else Ink.Line, ButtonShape)
        .padding(horizontal = 20.dp, vertical = 16.dp)
    @Composable
    fun Decorated(content: @Composable () -> Unit) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            if (icon != null) AkiIcon(icon, tint = if (focused) Ink.Accent else Ink.TextDim)
            Box { content() }
        }
    }
    if (!editing) {
        Box(
            Modifier.onFocusChanged { focused = it.isFocused }.clip(ButtonShape).clickable { editing = true }.then(frame),
        ) {
            Decorated {
                if (value.isEmpty()) Text(placeholder, style = Type.RowTitle.copy(color = Ink.TextFaint), maxLines = 1)
                else Text(if (secret) "\u2022".repeat(value.length) else value, style = Type.RowTitle, maxLines = 1)
            }
        }
    } else {
        BasicTextField(
            value = value,
            onValueChange = onChange,
            singleLine = true,
            textStyle = Type.RowTitle,
            cursorBrush = SolidColor(Ink.Accent),
            visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None,
            keyboardOptions = KeyboardOptions(keyboardType = if (secret) KeyboardType.Password else KeyboardType.Text, imeAction = action),
            keyboardActions = KeyboardActions(onSearch = { onAction() }, onDone = { onAction() }),
            modifier = Modifier
                .focusRequester(focus)
                .onFocusChanged {
                    focused = it.isFocused
                    if (it.isFocused) hadFocus = true else if (hadFocus) editing = false
                }
                // A text field keeps the Up and Down keys for itself: let them move the remote on, to what is above or below.
                .onPreviewKeyEvent { event ->
                    val up = event.key == Key.DirectionUp
                    val down = event.key == Key.DirectionDown
                    if ((up || down) && event.type == KeyEventType.KeyDown) focusManager.moveFocus(if (up) FocusDirection.Up else FocusDirection.Down)
                    up || down
                }
                .then(frame),
            decorationBox = { inner ->
                Decorated {
                    if (value.isEmpty()) Text(placeholder, style = Type.RowTitle.copy(color = Ink.TextFaint), maxLines = 1)
                    inner()
                }
            },
        )
    }
}

/** A big picture behind a screen, fading into the canvas on the left and at the bottom. If `picture` cannot be loaded, `fallback` is shown. */
@Composable
fun Backdrop(picture: String?, fallback: String? = null) {
    var failed by remember(picture) { mutableStateOf(false) }
    val shown = if (failed) fallback else picture
    Box(Modifier.fillMaxSize()) {
        if (shown != null) AsyncImage(
            model = shown,
            contentDescription = null,
            contentScale = ContentScale.Crop,
            alignment = Alignment.TopEnd,
            onSuccess = { Perf.once("first backdrop") },
            onError = { if (fallback != null && fallback != picture) failed = true },
            modifier = Modifier.fillMaxWidth().height(360.dp).align(Alignment.TopEnd),
        )
        Image(FadeToCanvas, contentDescription = null, contentScale = ContentScale.FillBounds, modifier = Modifier.fillMaxSize())
    }
}

/**
 * The fade over a backdrop: canvas colour, clear at the top right and solid at the left and the bottom. It is made once as a small
 * picture and stretched over the screen. Two gradients as large as the screen, blended on top of each other in every frame, made every
 * frame take 60 ms on the TV's graphics chip; one stretched picture takes 10 ms (see docs/design.md).
 */
private val FadeToCanvas: ImageBitmap by lazy {
    val width = 96
    val height = 54
    fun along(x: Float, stops: List<Pair<Float, Float>>): Float {
        val after = stops.indexOfFirst { it.first >= x }
        if (after <= 0) return stops.first().second
        val (x0, a0) = stops[after - 1]
        val (x1, a1) = stops[after]
        return a0 + (a1 - a0) * (x - x0) / (x1 - x0)
    }
    val pixels = IntArray(width * height) { i ->
        val sideways = along((i % width) / (width - 1f), listOf(0f to 0.94f, 0.45f to 0.94f, 0.8f to 0.45f, 1f to 0f))
        val downwards = along((i / width) / (height - 1f), listOf(0f to 0f, 0.3f to 0f, 0.62f to 1f, 1f to 1f))
        // Two layers of the canvas colour, one over the other.
        val alpha = 1f - (1f - sideways) * (1f - downwards)
        ((alpha * 255).toInt() shl 24) or (Ink.Canvas.toArgb() and 0xFFFFFF)
    }
    Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888).asImageBitmap()
}

/** A thin amber line for how far through something the viewer is. */
@Composable
fun Progress(share: Float, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(3.dp).background(Ink.Line)) { Box(Modifier.fillMaxWidth(share).height(3.dp).background(Ink.Accent)) }
}

/**
 * The home list moves by whole rows: when the remote lands on a card, the list scrolls until that card's row stands at the top
 * (`RowAtTop`), with the next row peeking in below it. This makes the row ask for its whole height, from its title down, not only
 * for the card to be seen.
 */
@Composable
fun Modifier.wholeRowInView(): Modifier = bringIntoViewResponder(remember {
    object : BringIntoViewResponder {
        override fun calculateRectForParent(localRect: Rect) = Rect(localRect.left, 0f, localRect.right, localRect.bottom)
        override suspend fun bringChildIntoView(localRect: () -> Rect?) {}
    }
})

/** Scrolls so the top of what was asked for goes to `top` (in pixels: the list's own top padding), instead of the least that shows it. */
class RowAtTop(private val top: Float) : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float) = offset - top
}

/**
 * The rule inside a row, sideways: scroll the least that shows the card in full, with `margin` to spare. A focused card grows a little
 * (and has a ring), and the row clips what grows past its edge: without the margin the card at the edge was cut off.
 */
class LeastToShow(private val margin: Float) : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float) = when {
        offset < margin -> offset - margin
        offset + size > containerSize - margin -> offset + size - containerSize + margin
        else -> 0f
    }
}

/** The margin a row keeps beside the focused card: more than the card grows (5 %) and the ring. */
@Composable
fun rememberLeastToShow(): LeastToShow {
    val margin = with(LocalDensity.current) { 12.dp.toPx() }
    return remember(margin) { LeastToShow(margin) }
}

/** The poster asks to be scrolled into view with 56 dp more below it, for the name that sits there (8 dp gap and a line of text). */
@Composable
private fun Modifier.roomForName(): Modifier {
    val extra = with(LocalDensity.current) { 56.dp.toPx() }
    return bringIntoViewResponder(remember(extra) {
        object : BringIntoViewResponder {
            override fun calculateRectForParent(localRect: Rect) = localRect.copy(bottom = localRect.bottom + extra)
            override suspend fun bringChildIntoView(localRect: () -> Rect?) {}
        }
    })
}
