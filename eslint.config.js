/**
 * ESLint 扁平配置（ESLint 10 起只认这一种格式）
 *
 * 组合关系：eslint-plugin-vue 的 flat/recommended（Vue3 SFC 解析 + 模板规则）
 * + @typescript-eslint 插件的 eslint-recommended/base/recommended 规则集
 * + eslint-config-prettier 置底（关掉与 Prettier 冲突的排版类规则，
 *   否则 --fix 会顺手重排全仓模板缩进）。
 *
 * 未引入 typescript-eslint / @eslint/js 元包：规则集直接取自已安装的
 * @typescript-eslint/eslint-plugin，避免为一份 lint 配置再加依赖。
 */
import vue from "eslint-plugin-vue";
import * as tsParser from "@typescript-eslint/parser";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import prettier from "eslint-config-prettier";

const TS_FILES = ["**/*.ts", "**/*.vue"];

export default [
  {
    ignores: [
      "dist/**",
      "target/**",
      "public/**",
      "design/**",
      "docs/**",
      "plans/**",
      "src-tauri/target/**",
      "src-tauri/resources/**",
      "src-tauri/sidecar-bin/**",
    ],
  },

  ...vue.configs["flat/recommended"],

  // TS 与 SFC 的 <script lang="ts"> 都走 TS parser；.vue 外层仍是 vue-eslint-parser，
  // 因此 .ts 覆盖 parser、.vue 只覆盖 parserOptions.parser（两者不能写在同一块里）
  {
    files: ["**/*.ts"],
    languageOptions: { parser: tsParser },
  },
  {
    files: ["**/*.vue"],
    languageOptions: { parserOptions: { parser: tsParser } },
  },
  {
    files: TS_FILES,
    plugins: { "@typescript-eslint": tsPlugin },
    rules: {
      ...tsPlugin.configs["eslint-recommended"].rules,
      ...tsPlugin.configs.base.rules,
      ...tsPlugin.configs.recommended.rules,

      // 标识符存在性由 vue-tsc 判定，含 Vue 编译器宏与 import.meta.env
      "no-undef": "off",
      // 路由页面统一 index.vue / 组件后缀 View，多词命名规则在此形态下无意义
      "vue/multi-word-component-names": "off",
      // IPC/事件回包在改造完成前先保留逃生口，由 vue-tsc 承担真实类型收窄
      "@typescript-eslint/no-explicit-any": "off",
      // 未用参数以下划线开头即视为占位
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // @ts-ignore 保留但必须写理由：跨端类型（node fetch body）等场景换成
      // @ts-expect-error 会在"该文件未被类型检查"时反成错误，不能盲切
      "@typescript-eslint/ban-ts-comment": [
        "error",
        { "ts-ignore": "allow-with-description" },
      ],
      // 存量债务：GroupEditModal 的 v-model 直接改 config prop（3 处）。
      // 保留规则为 warn 让债务可见，改成弹窗内草稿 + emit('save', draft) 后撤掉本行
      "vue/no-mutating-props": "warn",
      // 两处 v-html 注入的都是本仓生成的内容（SvgIcon 图标 path / flag.ts 国旗 SVG），
      // 非用户输入
      "vue/no-v-html": "off",
    },
  },

  prettier,
];
