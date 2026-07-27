<script setup lang="ts">
type Entry = { name: string; path: string; is_dir: boolean; is_image: boolean; has_docs: boolean };

const props = defineProps<{
  node: Entry;
  depth: number;
  openMap: Record<string, boolean>;
  childMap: Record<string, Entry[]>;
  activePath: string;
  docsOnly: boolean;
  ignored: string[];
}>();
const emit = defineEmits<{
  (e: "toggle", n: Entry): void;
  (e: "select", n: Entry): void;
  (e: "del", n: Entry): void;
  (e: "ctx", p: { node: Entry; e: MouseEvent }): void;
}>();

function isDoc(e: Entry): boolean {
  if (e.is_dir) return e.has_docs;
  return e.is_image || /\.(md|markdown|txt|pdf|html?)$/i.test(e.name);
}
function kids(): Entry[] {
  const list = props.childMap[props.node.path] || [];
  return props.docsOnly ? list.filter(isDoc) : list;
}
function onClick() {
  if (props.node.is_dir) emit("toggle", props.node);
  else emit("select", props.node);
}
</script>

<template>
  <div>
    <button
      class="row"
      :class="{ active: activePath === node.path, ignored: ignored.includes(node.path) }"
      :style="{ paddingLeft: 8 + depth * 13 + 'px' }"
      @click="onClick"
      @contextmenu.prevent="emit('ctx', { node, e: $event })"
    >
      <span class="ic">
        <template v-if="node.is_dir">{{ openMap[node.path] ? "▾" : "▸" }}</template>
        <template v-else-if="node.is_image">◧</template>
        <template v-else>·</template>
      </span>
      <span class="nm">{{ node.name }}</span>
      <span class="del" title="Eliminar" @click.stop="emit('del', node)">×</span>
    </button>
    <div v-if="node.is_dir && openMap[node.path]">
      <TreeNode
        v-for="c in kids()"
        :key="c.path"
        :node="c"
        :depth="depth + 1"
        :open-map="openMap"
        :child-map="childMap"
        :active-path="activePath"
        :docs-only="docsOnly"
        :ignored="ignored"
        @toggle="emit('toggle', $event)"
        @select="emit('select', $event)"
        @del="emit('del', $event)"
        @ctx="emit('ctx', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.row {
  display: flex; align-items: center; gap: 6px; width: 100%; text-align: left;
  border: none; background: transparent; color: var(--text); cursor: pointer;
  padding: 5px 8px; border-radius: 6px; font-size: 13px;
  white-space: nowrap; overflow: hidden; min-height: 28px; flex: none;
}
.row:hover { background: var(--border); }
.row.active { background: var(--accent-soft); color: var(--accent); font-weight: 500; }
.row.ignored { opacity: 0.4; }
.row.ignored .nm { text-decoration: line-through; }
.ic { width: 13px; flex: none; text-align: center; color: var(--muted); font-size: 11px; }
.nm { overflow: hidden; text-overflow: ellipsis; flex: 1; }
.del {
  flex: none; opacity: 0; color: var(--muted); font-size: 15px; line-height: 1;
  padding: 0 4px; border-radius: 4px;
}
.row:hover .del { opacity: 0.55; }
.del:hover { opacity: 1; color: #c0392b; background: rgba(192, 57, 43, 0.12); }
</style>
