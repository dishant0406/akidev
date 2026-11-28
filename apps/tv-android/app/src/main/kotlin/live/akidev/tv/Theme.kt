package live.akidev.tv

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/** The palette of design/DESIGN_PLAN.md (same values as apps/desktop/src/design/tokens.rs). Amber is only for focus, progress and the active item. */
object Ink {
    val Canvas = Color(0xFF0C0B0A)
    val Surface = Color(0xFF151413)
    val Raised = Color(0xFF1E1C1A)
    val Line = Color(0xFF2A2825)
    val Text = Color(0xFFF1EDE6)
    val TextDim = Color(0xFFA39D92)
    val TextFaint = Color(0xFF756F66)
    val Accent = Color(0xFFE6B450)
    val Danger = Color(0xFFE8786E)
}

private val Title = FontFamily(
    Font(R.font.shippori_regular, FontWeight.Normal),
    Font(R.font.shippori_semibold, FontWeight.SemiBold),
)
private val Ui = FontFamily(
    Font(R.font.hanken_regular, FontWeight.Normal),
    Font(R.font.hanken_medium, FontWeight.Medium),
    Font(R.font.hanken_semibold, FontWeight.SemiBold),
)

/** Sizes are for a TV seen from a sofa: nothing below 16 sp. Every style has a fixed line height, so a line takes the same room whatever it says. */
object Type {
    val Billboard = TextStyle(fontFamily = Title, fontWeight = FontWeight.SemiBold, fontSize = 36.sp, lineHeight = 50.sp, letterSpacing = (-0.5).sp, color = Ink.Text)
    val Display = TextStyle(fontFamily = Title, fontWeight = FontWeight.SemiBold, fontSize = 44.sp, lineHeight = 52.sp, color = Ink.Text)
    val RowTitle = TextStyle(fontFamily = Title, fontWeight = FontWeight.SemiBold, fontSize = 24.sp, lineHeight = 32.sp, color = Ink.Text)
    val Body = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Normal, fontSize = 18.sp, lineHeight = 26.sp, color = Ink.TextDim)
    val Label = TextStyle(fontFamily = Ui, fontWeight = FontWeight.SemiBold, fontSize = 18.sp, lineHeight = 24.sp, color = Ink.Text)
    /** The big number of a "Top 10" card. Faint, so the poster beside it is what the eye lands on. */
    val Rank = TextStyle(fontFamily = Title, fontWeight = FontWeight.SemiBold, fontSize = 96.sp, lineHeight = 104.sp, letterSpacing = (-4).sp, color = Ink.TextFaint)
    val Meta = TextStyle(fontFamily = Ui, fontWeight = FontWeight.Medium, fontSize = 16.sp, lineHeight = 22.sp, letterSpacing = 1.sp, color = Ink.TextFaint)
}
