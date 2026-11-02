target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'main_syntax'
source_filename = "main_syntax"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.0 = private unnamed_addr constant [9 x i8] c"127.0.0.1", align 1

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
declare void @ignite_rt_fail(ptr) noreturn
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
declare void @"tests.syntax.Runner.main"(ptr, ptr)

define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
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
  br label %done
done:
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
  %fits = icmp ult i64 %end, %li
  %ok = and i1 %small, %fits
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
  %t2 = alloca [32 x i8], align 8
  %t3 = alloca [24 x i8], align 8
  %t6 = alloca [24 x i8], align 8
  %t8 = alloca [40 x i8], align 8
  call void @ignite_rt_init(i32 %argc, ptr %argv), !dbg !5
  store ptr @.str.0, ptr %t3, align 8, !dbg !6
  %t4 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !6
  store i64 9, ptr %t4, align 8, !dbg !6
  %t5 = getelementptr inbounds i8, ptr %t3, i64 16, !dbg !6
  store i64 9, ptr %t5, align 8, !dbg !6
  call void @"copy.Text"(ptr %t6, ptr %t3), !dbg !6
  call void @llvm.memcpy.p0.p0.i64(ptr %t2, ptr %t6, i64 24, i1 false), !dbg !5
  %t7 = getelementptr inbounds i8, ptr %t2, i64 24, !dbg !5
  store i64 1, ptr %t7, align 8, !dbg !5
  call void @"tests.syntax.Runner.main"(ptr %t8, ptr %t2), !dbg !5
  %t9 = load i32, ptr %t8, align 4, !dbg !5
  %t10 = icmp eq i32 %t9, 1, !dbg !5
  br i1 %t10, label %L11, label %L12, !dbg !5
L11:
  %t13 = getelementptr inbounds i8, ptr %t8, i64 8, !dbg !5
  call void @ignite_rt_fail(ptr %t13), !dbg !5
  unreachable, !dbg !5
L12:
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
!llvm.module.flags = !{!8, !9}
!0 = !DIFile(filename: "main_syntax.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "syntax.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "main", linkageName: "main", scope: !3, file: !3, line: 191, type: !2, scopeLine: 191, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 191, column: 3, scope: !4)
!6 = !DILocation(line: 191, column: 40, scope: !4)
!7 = !DILocation(line: 191, column: 60, scope: !4)
!8 = !{i32 7, !"Dwarf Version", i32 5}
!9 = !{i32 2, !"Debug Info Version", i32 3}
