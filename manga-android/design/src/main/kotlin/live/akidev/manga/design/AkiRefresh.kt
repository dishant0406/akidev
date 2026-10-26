package live.akidev.manga.design

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import kotlin.math.min
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private enum class Phase { Idle, Loading, Done }

/**
 * Pull down to run `onRefresh`, with Akidev's own indicator: a glass bead drops in as you pull, a yellow ring fills as you near the
 * line, it gives a tap when it is armed, spins while `onRefresh` runs, and shows a tick when it is done. Whatever scrolls inside is
 * what is pulled.
 */
@Composable
fun AkiRefreshable(onRefresh: suspend () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val density = LocalDensity.current
    val line = with(density) { 78.dp.toPx() }
    val haptic = LocalHapticFeedback.current
    val scope = rememberCoroutineScope()
    val pull = remember { Animatable(0f) }
    var phase by remember { mutableStateOf(Phase.Idle) }
    var armed by remember { mutableStateOf(false) }
    val turn = remember { mutableFloatStateOf(0f) }

    fun start() {
        phase = Phase.Loading
        scope.launch {
            launch { while (phase == Phase.Loading) { turn.floatValue = (turn.floatValue + 14f) % 360f; delay(16) } }
            pull.animateTo(0f, AkiMotion.settle())
            onRefresh()
            phase = Phase.Done
            delay(700)
            phase = Phase.Idle
        }
    }

    val connection = remember {
        object : NestedScrollConnection {
            // Pulling back up first takes the pull away, then scrolls the page.
            override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                if (phase != Phase.Idle || source != NestedScrollSource.UserInput || pull.value <= 0f || available.y >= 0f) return Offset.Zero
                val taken = maxOf(available.y, -pull.value)
                scope.launch { pull.snapTo(pull.value + taken) }
                return Offset(0f, taken)
            }

            // What the page could not scroll (it is at the top) becomes the pull, at half the finger's speed.
            override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
                if (phase != Phase.Idle || source != NestedScrollSource.UserInput || available.y <= 0f) return Offset.Zero
                scope.launch {
                    pull.snapTo(pull.value + available.y * 0.5f)
                    val now = pull.value >= line
                    if (now && !armed) haptic.performHapticFeedback(HapticFeedbackType.ContextClick)
                    armed = now
                }
                return Offset(0f, available.y)
            }

            // Letting go past the line starts the refresh.
            override suspend fun onPreFling(available: Velocity): Velocity {
                if (phase == Phase.Idle && pull.value > 0f) {
                    if (armed) start() else scope.launch { pull.animateTo(0f, AkiMotion.settle()) }
                    armed = false
                }
                return Velocity.Zero
            }
        }
    }

    Box(modifier.nestedScroll(connection)) {
        content()
        Bead(pull.value, line, phase, turn.floatValue, Modifier.align(Alignment.TopCenter))
    }
}

@Composable
private fun Bead(pull: Float, line: Float, phase: Phase, turn: Float, modifier: Modifier) {
    val colors = Aki.colors
    val progress = if (phase == Phase.Idle) min(pull / line, 1f) else 1f
    val density = LocalDensity.current
    // Where the bead sits below the top: dragged along with the pull (slower than the finger), then held in place.
    val drop = if (phase == Phase.Idle) min(pull * 0.5f, with(density) { 52.dp.toPx() }) - with(density) { 44.dp.toPx() } else with(density) { 10.dp.toPx() }
    Box(
        modifier.statusBarsPadding().offset { androidx.compose.ui.unit.IntOffset(0, drop.toInt()) }
            .alpha(if (phase == Phase.Idle) min(pull / with(density) { 36.dp.toPx() }, 1f) else 1f)
            .scale(if (phase == Phase.Idle) 0.55f + 0.45f * progress else 1f)
            .size(46.dp).akiGlass(AkiShapes.pill(46.dp)),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(28.dp)) {
            val width = 3.dp.toPx()
            val box = Size(size.width - width, size.height - width)
            val corner = Offset(width / 2, width / 2)
            drawCircle(colors.secondary, style = Stroke(width))
            when (phase) {
                Phase.Idle -> drawArc(colors.primary, -90f, 360f * progress, false, corner, box, style = Stroke(width, cap = StrokeCap.Round))
                Phase.Loading -> drawArc(colors.primary, turn, 100f, false, corner, box, style = Stroke(width, cap = StrokeCap.Round))
                Phase.Done -> drawCircle(colors.primary, style = Stroke(width))
            }
        }
        if (phase == Phase.Done) AkiIcon("tick", size = 17.dp)
        if (phase == Phase.Idle) AkiIcon("refresh", Modifier.rotate(progress * 180f).alpha(0.4f + 0.6f * progress), size = 15.dp, tone = AkiTone.Muted)
    }
}
