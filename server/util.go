// util.go —— 小工具：yaml 解析包装 + 稳定排序辅助。
package main

import (
	"sort"

	"gopkg.in/yaml.v3"
)

func yamlUnmarshal(data []byte, v any) error {
	return yaml.Unmarshal(data, v)
}

func boolToInt(b bool) int {
	if b {
		return 1
	}
	return 0
}

// stableSortBy 用三段比较函数对 cards 做稳定排序（cmp<0 → a 在前）。
func stableSortBy(cards []ProjectCard, cmp func(a, b ProjectCard) int) {
	sort.SliceStable(cards, func(i, j int) bool {
		return cmp(cards[i], cards[j]) < 0
	})
}
