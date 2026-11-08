target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'main_hub'
source_filename = "main_hub"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.0 = private unnamed_addr constant [9 x i8] c"127.0.0.1", align 1
@.str.2 = private unnamed_addr constant [1 x i8] c":", align 1
@.str.3 = private unnamed_addr constant [6 x i8] c"hello\0A", align 1
@.str.4 = private unnamed_addr constant [3 x i8] c"hub", align 1
@.str.1 = private unnamed_addr constant [13 x i8] c"listening on ", align 1

declare void @ignite_rt_init(i32, ptr)
declare ptr @ignite_rt_alloc(i64, i64)
declare i8 @ignite_rt_spawn(ptr, ptr)
declare void @ignite_rt_spawn_ready()
declare void @ignite_rt_task_end()
declare i64 @ignite_rt_arena_mark()
declare void @ignite_rt_arena_release(i64)
declare i8 @ignite_rt_arena_due(i64, i64)
declare void @ignite_rt_arena_scratch(i8)
declare void @ignite_rt_arena_scratch_clear()
declare ptr @ignite_rt_arena_state() nounwind willreturn speculatable memory(none)
declare i32 @ignite_rt_args_port(ptr, ptr, ptr)
declare void @ignite_rt_exit(i32) noreturn
declare void @ignite_rt_fail(ptr) cold noreturn
declare void @ignite_rt_log(ptr)
declare i32 @ignite_rt_tcp_bind(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_accept(i64, ptr, ptr)
declare i64 @ignite_rt_tcp_local_port(i64)
declare i32 @ignite_rt_tcp_connect(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_read(i64, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_write_all(i64, ptr, ptr)
declare i32 @ignite_rt_tcp_close(i64, ptr)
declare void @ignite_rt_tcp_release(i64)
declare i32 @ignite_rt_stdin_read(i64, ptr, ptr)
declare i32 @ignite_rt_stdin_is_terminal(ptr, ptr)
declare i32 @ignite_rt_stdin_set_raw(i8, ptr)
declare i32 @ignite_rt_stdout_write_all(ptr, ptr)
declare i32 @ignite_rt_stderr_write_all(ptr, ptr)
declare void @ignite_rt_io_error_new(ptr, ptr)
declare void @ignite_rt_text_from_bytes_lossy(ptr, ptr)
declare void @ignite_rt_text_trim(ptr, ptr)
declare void @ignite_rt_text_concat(ptr, ptr, ptr)
declare i8 @ignite_rt_text_eq(ptr, ptr)
declare i8 @ignite_rt_text_starts_with(ptr, ptr)
declare void @ignite_rt_text_join(ptr, ptr, ptr)
declare void @ignite_rt_int_to_text(ptr, i64)
declare i8 @ignite_rt_int_parse(ptr, ptr)
declare i32 @ignite_rt_tcp_set_idle_timeout(i64, i64, ptr)
declare ptr @ignite_rt_store_new(ptr, ptr, ptr, i64, i64)
declare i64 @ignite_rt_store_next_id(ptr)
declare void @ignite_rt_store_put(ptr, i64, ptr)
declare i8 @ignite_rt_store_get(ptr, i64, ptr)
declare i8 @ignite_rt_store_remove(ptr, i64, ptr)
declare void @ignite_rt_store_values(ptr, ptr)
declare i8 @ignite_rt_store_update(ptr, i64, ptr, ptr, ptr)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)
declare void @"tests.bounds.Hub.main"(ptr, ptr)

define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) alwaysinline {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
  call void @ignite.grow(ptr %list, i64 %need, i64 %size, i64 %align)
  br label %done
done:
  ret void
}

define internal void @ignite.grow(ptr %list, i64 %need, i64 %size, i64 %align) noinline cold {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %double = mul i64 %cap, 2
  %big = icmp sgt i64 %double, %need
  %c1 = select i1 %big, i64 %double, i64 %need
  %small = icmp slt i64 %c1, 4
  %newcap = select i1 %small, i64 4, i64 %c1
  %bytes = mul i64 %newcap, %size
  %new = call ptr @ignite.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  ret void
}

define internal ptr @ignite.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %limp = getelementptr inbounds i8, ptr %st, i64 8
  %lim = load ptr, ptr %limp, align 8
  %ci = ptrtoint ptr %cur to i64
  %li = ptrtoint ptr %lim to i64
  %am1 = sub i64 %align, 1
  %up = add i64 %ci, %am1
  %mask = sub i64 0, %align
  %start = and i64 %up, %mask
  %end = add i64 %start, %size
  %small = icmp ult i64 %size, 65536
  %fits = icmp ule i64 %end, %li
  %live = icmp ne i64 %li, 0
  %fit = and i1 %fits, %live
  %ok = and i1 %small, %fit
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @ignite_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @ignite.arena_mark() alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @ignite.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @ignite.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @ignite.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  %nothing = icmp sge i64 %mark, %pos
  br i1 %nothing, label %done, label %down
down:
  %basep = getelementptr inbounds i8, ptr %st, i64 24
  %base = load ptr, ptr %basep, align 8
  %bi = ptrtoint ptr %base to i64
  %bpos = add i64 %bi, %off
  %inside = icmp sge i64 %mark, %bpos
  br i1 %inside, label %chunk, label %slow
chunk:
  %back = sub i64 %mark, %bpos
  %new = getelementptr i8, ptr %base, i64 %back
  store ptr %new, ptr %st, align 8
  br label %done
slow:
  call void @ignite_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}

define i32 @main(i32 %argc, ptr %argv) !dbg !4 {
entry:
  %t1 = alloca [32 x i8], align 8
  %t2 = alloca [8 x i8], align 8
  %t3 = alloca [4 x i8], align 4
  %t12 = alloca [24 x i8], align 8
  %t15 = alloca [24 x i8], align 8
  %t17 = alloca [8 x i8], align 8
  %t25 = alloca [24 x i8], align 8
  %t28 = alloca [24 x i8], align 8
  %t31 = alloca [24 x i8], align 8
  %t32 = alloca [24 x i8], align 8
  %t33 = alloca [24 x i8], align 8
  %t34 = alloca [24 x i8], align 8
  %t35 = alloca [8 x i8], align 8
  %t36 = alloca [56 x i8], align 8
  %t38 = alloca [24 x i8], align 8
  %t41 = alloca [24 x i8], align 8
  %t43 = alloca [24 x i8], align 8
  %t46 = alloca [24 x i8], align 8
  %t48 = alloca [40 x i8], align 8
  call void @ignite_rt_init(i32 %argc, ptr %argv), !dbg !5
  %t4 = call i32 @ignite_rt_args_port(ptr %t2, ptr %t3, ptr %t1), !dbg !5
  %t5 = icmp ne i32 %t4, 0, !dbg !5
  br i1 %t5, label %L6, label %L7, !dbg !5
L6:
  %t8 = getelementptr inbounds i8, ptr %t1, i64 24, !dbg !5
  store i32 2, ptr %t8, align 4, !dbg !5
  call void @ignite_rt_fail(ptr %t1), !dbg !5
  unreachable, !dbg !5
L7:
  %t9 = load i32, ptr %t3, align 4, !dbg !5
  %t10 = icmp ne i32 %t9, 0, !dbg !5
  %t11 = load i64, ptr %t2, align 8, !dbg !5
  store ptr @.str.0, ptr %t12, align 8, !dbg !6
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !6
  store i64 9, ptr %t13, align 8, !dbg !6
  %t14 = getelementptr inbounds i8, ptr %t12, i64 16, !dbg !6
  store i64 9, ptr %t14, align 8, !dbg !6
  call void @"copy.Text"(ptr %t15, ptr %t12), !dbg !6
  %t16 = select i1 %t10, i64 %t11, i64 7401, !dbg !5
  %t18 = call i32 @ignite_rt_tcp_bind(ptr %t15, i64 %t16, ptr %t17, ptr %t1), !dbg !5
  %t19 = icmp ne i32 %t18, 0, !dbg !5
  br i1 %t19, label %L20, label %L21, !dbg !5
L20:
  %t22 = getelementptr inbounds i8, ptr %t1, i64 24, !dbg !5
  store i32 1, ptr %t22, align 4, !dbg !5
  call void @ignite_rt_fail(ptr %t1), !dbg !5
  unreachable, !dbg !5
L21:
  %t23 = load i64, ptr %t17, align 8, !dbg !5
  %t24 = call i64 @ignite_rt_tcp_local_port(i64 %t23), !dbg !5
  store ptr @.str.1, ptr %t25, align 8, !dbg !5
  %t26 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !5
  store i64 13, ptr %t26, align 8, !dbg !5
  %t27 = getelementptr inbounds i8, ptr %t25, i64 16, !dbg !5
  store i64 13, ptr %t27, align 8, !dbg !5
  store ptr @.str.2, ptr %t28, align 8, !dbg !5
  %t29 = getelementptr inbounds i8, ptr %t28, i64 8, !dbg !5
  store i64 1, ptr %t29, align 8, !dbg !5
  %t30 = getelementptr inbounds i8, ptr %t28, i64 16, !dbg !5
  store i64 1, ptr %t30, align 8, !dbg !5
  call void @ignite_rt_int_to_text(ptr %t31, i64 %t24), !dbg !5
  call void @ignite_rt_text_concat(ptr %t32, ptr %t25, ptr %t15), !dbg !5
  call void @ignite_rt_text_concat(ptr %t33, ptr %t32, ptr %t28), !dbg !5
  call void @ignite_rt_text_concat(ptr %t34, ptr %t33, ptr %t31), !dbg !5
  call void @ignite_rt_log(ptr %t34), !dbg !5
  store i64 %t23, ptr %t35, align 8, !dbg !5
  %t37 = load i64, ptr %t35, align 8, !dbg !8
  store i64 %t37, ptr %t36, align 8, !dbg !5
  store ptr @.str.3, ptr %t38, align 8, !dbg !9
  %t39 = getelementptr inbounds i8, ptr %t38, i64 8, !dbg !9
  store i64 6, ptr %t39, align 8, !dbg !9
  %t40 = getelementptr inbounds i8, ptr %t38, i64 16, !dbg !9
  store i64 6, ptr %t40, align 8, !dbg !9
  call void @"copy.Text"(ptr %t41, ptr %t38), !dbg !9
  %t42 = getelementptr inbounds i8, ptr %t36, i64 8, !dbg !5
  call void @llvm.memcpy.p0.p0.i64(ptr %t42, ptr %t41, i64 24, i1 false), !dbg !5
  store ptr @.str.4, ptr %t43, align 8, !dbg !10
  %t44 = getelementptr inbounds i8, ptr %t43, i64 8, !dbg !10
  store i64 3, ptr %t44, align 8, !dbg !10
  %t45 = getelementptr inbounds i8, ptr %t43, i64 16, !dbg !10
  store i64 3, ptr %t45, align 8, !dbg !10
  call void @"copy.Text"(ptr %t46, ptr %t43), !dbg !10
  %t47 = getelementptr inbounds i8, ptr %t36, i64 32, !dbg !5
  call void @llvm.memcpy.p0.p0.i64(ptr %t47, ptr %t46, i64 24, i1 false), !dbg !5
  call void @"tests.bounds.Hub.main"(ptr %t48, ptr %t36), !dbg !5
  %t49 = load i32, ptr %t48, align 4, !dbg !5
  %t50 = icmp eq i32 %t49, 1, !dbg !5
  br i1 %t50, label %L51, label %L52, !dbg !5
L51:
  %t53 = getelementptr inbounds i8, ptr %t48, i64 8, !dbg !5
  call void @ignite_rt_fail(ptr %t53), !dbg !5
  unreachable, !dbg !5
L52:
  call void @ignite_rt_exit(i32 0), !dbg !5
  unreachable, !dbg !5
}

define internal void @"copy.Text"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @ignite.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 1
  %t11 = call ptr @ignite.alloc(i64 %t10, i64 1)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}

!llvm.dbg.cu = !{!1}
!llvm.module.flags = !{!11, !12}
!0 = !DIFile(filename: "main_hub.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "bounds.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "main", linkageName: "main", scope: !3, file: !3, line: 46, type: !2, scopeLine: 46, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 46, column: 3, scope: !4)
!6 = !DILocation(line: 45, column: 45, scope: !4)
!7 = !DILocation(line: 45, column: 65, scope: !4)
!8 = !DILocation(line: 46, column: 35, scope: !4)
!9 = !DILocation(line: 46, column: 55, scope: !4)
!10 = !DILocation(line: 46, column: 79, scope: !4)
!11 = !{i32 7, !"Dwarf Version", i32 5}
!12 = !{i32 2, !"Debug Info Version", i32 3}
