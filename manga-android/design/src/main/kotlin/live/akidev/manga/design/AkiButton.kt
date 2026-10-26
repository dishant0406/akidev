package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.unit.dp

enum class AkiButtonVariant {
    /** The main action: yellow glass. */
    Primary, Glass, Ghost, Destructive
}

enum class AkiButtonSize(val height: androidx.compose.ui.unit.Dp) { Small(44.dp), Regular(48.dp), Large(56.dp) }

@Composable
fun AkiButton(
    title: String,
    modifier: Modifier = Modifier,
    icon: String? = null,
    variant: AkiButtonVariant = AkiButtonVariant.Glass,
    size: AkiButtonSize = AkiButtonSize.Regular,
    fullWidth: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val tone = when (variant) {
        AkiButtonVariant.Primary -> AkiTone.OnPrimary
        AkiButtonVariant.Destructive -> AkiTone.Destructive
        else -> AkiTone.Text
    }
    val face = if (variant == AkiButtonVariant.Ghost) Modifier else Modifier.akiGlass(AkiShapes.pill(size.height), tint = if (variant == AkiButtonVariant.Primary) Aki.colors.primary else null)
    Row(
        modifier
            .then(if (fullWidth) Modifier.fillMaxWidth() else Modifier)
            .height(size.height)
            .then(face)
            .alpha(if (enabled) 1f else 0.5f)
            .akiPress(label = title, enabled = enabled, onClick = onClick)
            .padding(horizontal = 20.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) AkiIcon(icon, size = 15.dp, tone = tone)
        AkiText(title, AkiTextStyle.BodyStrong, tone = tone, lines = 1)
    }
}
