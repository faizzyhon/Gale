package com.faizzyhon.gale.engine

/**
 * Ad and tracker domains (assets/adblock-hosts.txt, from StevenBlack/hosts). A host is blocked when it or any parent
 * domain is on the list, so "pagead2.googlesyndication.com" matches "googlesyndication.com".
 */
class AdBlock(lines: Sequence<String>) {
    private val hosts: Set<String> = lines.map { it.trim() }.filter { it.isNotEmpty() && it[0] != '#' }.toHashSet()

    val size get() = hosts.size

    fun blocks(host: String?): Boolean {
        var h = host?.lowercase()?.trimEnd('.') ?: return false
        while (true) {
            if (h in hosts) return true
            val dot = h.indexOf('.')
            if (dot < 0 || h.indexOf('.', dot + 1) < 0) return false // stop before the bare TLD
            h = h.substring(dot + 1)
        }
    }
}
