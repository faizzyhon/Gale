package com.faizzyhon.gale.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/** The desktop app's colour tokens (src/styles/index.css), so both apps look like one product. */
@Immutable
data class GaleColors(
    val app: Color, val pane: Color, val surface: Color, val line: Color,
    val fg: Color, val fg2: Color, val fg3: Color, val accent: Color, val accentFg: Color,
    val ok: Color, val warn: Color, val bad: Color,
)

val Light = GaleColors(
    app = Color(0xFFF3F3F3), pane = Color(0xFFEBEBEB), surface = Color(0xFFFFFFFF), line = Color(0x14000000),
    fg = Color(0xFF1B1B1B), fg2 = Color(0xFF5C5C5C), fg3 = Color(0xFF8A8A8A), accent = Color(0xFF005FB8), accentFg = Color.White,
    ok = Color(0xFF0F7B0F), warn = Color(0xFF9D5D00), bad = Color(0xFFC42B1C),
)

val Dark = GaleColors(
    app = Color(0xFF202020), pane = Color(0xFF1A1A1A), surface = Color(0xFF2B2B2B), line = Color(0x14FFFFFF),
    fg = Color.White, fg2 = Color(0xFFC5C5C5), fg3 = Color(0xFF8F8F8F), accent = Color(0xFF60CDFF), accentFg = Color(0xFF003A5C),
    ok = Color(0xFF6CCB5F), warn = Color(0xFFFCE100), bad = Color(0xFFFF99A4),
)

val LocalGale = staticCompositionLocalOf { Light }

@Composable
fun GaleTheme(theme: String, content: @Composable () -> Unit) {
    val dark = when (theme) { "dark" -> true; "light" -> false; else -> isSystemInDarkTheme() }
    val c = if (dark) Dark else Light
    val scheme = (if (dark) darkColorScheme() else lightColorScheme()).copy(
        primary = c.accent, onPrimary = c.accentFg, primaryContainer = c.accent.copy(alpha = 0.16f), onPrimaryContainer = c.fg,
        secondaryContainer = c.accent.copy(alpha = 0.16f), onSecondaryContainer = c.fg,
        background = c.app, onBackground = c.fg, surface = c.surface, onSurface = c.fg, surfaceVariant = c.pane,
        onSurfaceVariant = c.fg2, outline = c.line, outlineVariant = c.line, error = c.bad,
        surfaceContainer = c.surface, surfaceContainerLow = c.surface, surfaceContainerHigh = c.surface, surfaceContainerHighest = c.pane,
    )
    androidx.compose.runtime.CompositionLocalProvider(LocalGale provides c) {
        MaterialTheme(
            colorScheme = scheme,
            shapes = Shapes(extraSmall = RoundedCornerShape(4.dp), small = RoundedCornerShape(4.dp), medium = RoundedCornerShape(8.dp), large = RoundedCornerShape(12.dp)),
            content = content,
        )
    }
}
