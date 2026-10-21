#!/bin/bash
# Screens use the components in App/UI and nothing native: no Text, Button, TextField, Image, Toggle, Picker, Menu, Label,
# ProgressView, List, Form, Section, Divider, alert, sheet, NavigationLink, and no font, colour or glass modifiers outside
# App/UI. Layout containers (VStack, HStack, ZStack, ScrollView, LazyVGrid, ForEach) are not controls and stay allowed.
set -euo pipefail

cd "$(dirname "$0")/../App"
patterns='\bText\(|\bButton\(|\bTextField\(|\bSecureField\(|\bImage\(|\bToggle\(|\bPicker\(|\bMenu\b|\bLabel\(|\bProgressView\(|\bList\b|\bForm\b|\bSection\(|\bDivider\(|\bSlider\(|\bStepper\(|\bTextEditor\(|\bLink\(|NavigationLink|\.alert\(|\.sheet\(|\.fullScreenCover\(|\.confirmationDialog\(|\.contextMenu\(|\.font\(|\.foregroundStyle\(|\.glassEffect\(|\.buttonStyle\('

found=$(grep -rnE "$patterns" --include='*.swift' . | grep -v '^\./UI/' || true)
if [ -n "$found" ]; then
  echo "Native controls or styling outside App/UI (use the Aki components):"
  echo "$found"
  exit 1
fi
echo "ok: screens use only Aki components"
