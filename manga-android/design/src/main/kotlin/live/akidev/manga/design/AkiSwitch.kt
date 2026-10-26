package live.akidev.manga.design

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp

/** A setting that is on or off, with a title and a line under it. */
@Composable
fun AkiSwitch(title: String, isOn: Boolean, onChange: (Boolean) -> Unit, modifier: Modifier = Modifier, subtitle: String? = null) {
    Row(
        modifier.toggleable(isOn, role = Role.Switch, onValueChange = onChange),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            AkiText(title, AkiTextStyle.BodyStrong)
            if (subtitle != null) AkiText(subtitle, AkiTextStyle.Caption, tone = AkiTone.Muted)
        }
        AkiSwitchKnob(isOn)
    }
}

@Composable
private fun AkiSwitchKnob(isOn: Boolean) {
    val colors = Aki.colors
    val track by animateColorAsState(if (isOn) colors.primary else colors.secondary, AkiMotion.settle(), label = "track")
    val offset by animateDpAsState(if (isOn) 22.dp else 2.dp, AkiMotion.settle(), label = "knob")
    Box(Modifier.size(width = 51.dp, height = 31.dp).background(track, CircleShape)) {
        Box(Modifier.offset { IntOffset(offset.roundToPx(), 2.dp.roundToPx()) }.size(27.dp).background(Color.White, CircleShape))
    }
}
