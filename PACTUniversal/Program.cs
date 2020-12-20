using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace PACTUniversal
{
    public class Program
    {
        public static void Main()
        {
            List<PerformanceCounter> perfs = new List<PerformanceCounter>();
            for (int i = 0; i < Environment.ProcessorCount; i++)
            {
                perfs.Add(new PerformanceCounter("Processor", "% Processor Time", $"{i}"));
            }

            while (true)
            {
                Console.Clear();
                foreach (var item in perfs)
                {
                    Console.WriteLine(item.NextValue());
                }
                System.Threading.Thread.Sleep(500);
            }
        }

    }
}
