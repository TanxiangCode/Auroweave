/**
 * 自定义区域规则 CRUD Hook
 * 作者: TanXiang
 *
 * 职责：管理自定义区域规则的新增、编辑、删除、快捷填充
 */
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useProxyStore } from "@/stores/proxy.store";
import { useToast } from "@/composables/useToast";
import type { CustomGroupRule } from "@/types";
import { builtinRegions } from "../utils/builtin-regions";

/**
 * 自定义区域规则管理 Hook
 */
export function useRegionRules() {
  const proxyStore = useProxyStore();
  const toast = useToast();

  /** 区域管理弹窗可见性 */
  const showRegionModal = ref(false);
  /** 当前编辑的规则 */
  const editingRule = ref<CustomGroupRule | null>(null);
  /** 是否为新增规则（vs 编辑已有规则） */
  const isNewRule = ref(false);

  /** 打开区域管理弹窗 */
  function openRegionModal() {
    showRegionModal.value = true;
  }

  /** 开始新增规则 */
  function addNewRule() {
    editingRule.value = {
      id: "",
      name: "",
      enabled: true,
      match_type: "keyword",
      group_type: "virtual",
      keywords: [],
      pattern: "",
      protocols: [],
      // 解锁匹配默认找 Gemini 可用节点（用户可改服务与期望状态）
      unlock: { service: "gemini", status: "yes" },
      order: proxyStore.customGroupRules.length,
    };
    isNewRule.value = true;
  }

  /** 开始编辑已有规则 */
  function editRule(rule: CustomGroupRule) {
    // 旧规则（迁移自 localStorage）可能无 group_type，默认 virtual 保证类型下拉正确回显
    editingRule.value = { group_type: "virtual", ...rule };
    isNewRule.value = false;
  }

  /** 应用内置区域快捷填充 */
  function applyBuiltinRegion(region: { name: string; keywords: string[] }) {
    if (!editingRule.value) return;
    editingRule.value.name = region.name;
    editingRule.value.match_type = "keyword";
    editingRule.value.keywords = [...region.keywords];
  }

  /** 保存规则（新增或更新）；含真实组类型时持久化后带规则重建内核 */
  async function saveRule() {
    if (!editingRule.value) return;
    if (!editingRule.value.name.trim()) {
      toast.warning("请填写区域名称");
      return;
    }
    // unlock 匹配的完整性校验：缺服务/状态时规则永远匹配不到节点
    if (editingRule.value.match_type === "unlock") {
      if (!editingRule.value.unlock?.service || !editingRule.value.unlock?.status) {
        toast.warning("请选择检测服务与期望状态");
        return;
      }
    }
    if (isNewRule.value) {
      const { id, ...ruleData } = editingRule.value;
      await proxyStore.addCustomGroupRule(ruleData);
    } else {
      await proxyStore.updateCustomGroupRule(editingRule.value);
    }
    editingRule.value = null;
    toast.success("区域规则已保存");
    await rebuildIfRealGroups();
  }

  /** 删除规则（若删除的是真实组规则，同样带规则重建内核以移除对应组） */
  async function deleteRule(id: string) {
    const removed = proxyStore.customGroupRules.find((r) => r.id === id);
    await proxyStore.deleteCustomGroupRule(id);
    toast.success("区域规则已删除");
    if (removed && (removed.group_type ?? "virtual") !== "virtual") {
      await rebuildIfRealGroups();
      toast.info("内核已重启", "对应自定义分组已从节点列表移除");
    }
  }

  /**
   * 存在真实组类型规则时：invoke custom_groups_apply 并显式传规则（后端先落盘再重建），
   * 确保自定义策略组进入内核且 settings.json 与前端状态一致
   */
  async function rebuildIfRealGroups() {
    const rules = proxyStore.customGroupRules;
    const hasRealGroup = rules.some(
      (r) => r.enabled && (r.group_type ?? "virtual") !== "virtual"
    );
    if (!hasRealGroup) return;
    try {
      const res: any = await invoke("custom_groups_apply", {
        rules: JSON.parse(JSON.stringify(rules)),
      });
      if (res?.success) {
        toast.success(
          "真实策略组已生成",
          "内核已重启，自定义分组已可在节点列表中查看与切换"
        );
      } else {
        toast.error("自定义分组生成失败", res?.error ?? "未知错误");
      }
    } catch (e) {
      toast.error("自定义分组生成失败", e instanceof Error ? e.message : String(e));
    }
  }

  /**
   * 从分组卡片直接编辑规则：按名称打开弹窗并进入编辑态
   * （真实组 tag 为 custom-{name}，虚拟组 tag 为 custom:{name}，统一去前缀查规则）
   */
  function editRuleByTag(tag: string) {
    const name = tag.replace(/^custom[:-]/, "");
    const rule = proxyStore.customGroupRules.find(
      (r) => r.name === name
    );
    if (!rule) {
      toast.warning("未找到对应规则，可能已被删除");
      return;
    }
    editRule(rule);
    showRegionModal.value = true;
  }

  /** 关闭区域管理弹窗 */
  function closeRegionModal() {
    showRegionModal.value = false;
  }

  return {
    // 状态
    showRegionModal,
    editingRule,
    isNewRule,
    builtinRegions,
    // 方法
    openRegionModal,
    addNewRule,
    editRule,
    editRuleByTag,
    applyBuiltinRegion,
    saveRule,
    deleteRule,
    closeRegionModal,
    rebuildIfRealGroups,
  };
}
